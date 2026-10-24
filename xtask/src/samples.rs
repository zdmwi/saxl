//! `cargo xtask gen-samples`: random saxl files built from the grammar.
//!
//! The samples exist to cover every construct in training data, not to look
//! like real architectures. Each file is built to satisfy the validator: names
//! are declared before use, each position gets the right kind of element,
//! synchronous calls form no cycle, every element takes part in something, and
//! every scenario step follows a relation. The text then goes through the
//! formatter, and files that still have errors are dropped and counted.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use saxl_core::{FileId, Severity};

const USAGE: &str = "usage: cargo xtask gen-samples --out <dir> [--count <n>] [--seed <n>]";

/// Every construct the generator can emit. The tests require all of them.
pub(crate) const CONSTRUCTS: &[&str] = &[
    "root:system",
    "root:library",
    "element:actor",
    "element:external",
    "element:service",
    "element:store",
    "element:queue",
    "element:consumer",
    "element:package",
    "element:api",
    "element:module",
    "element:hook",
    "description",
    "unicode",
    "comment",
    "relation:sync",
    "relation:async",
    "relation:chain",
    "relation:mixed_chain",
    "relation:description",
    "impl",
    "at",
    "data:in",
    "data:cached",
    "data:via",
    "data:pub",
    "data:multiline",
    "deploy",
    "deploy:multi_region",
    "deploy:cdn",
    "deploy:cluster",
    "deploy:managed",
    "workload:list",
    "attr:replicas",
    "attr:autoscale",
    "attr:primary_replica",
    "publish",
    "publish:crate",
    "publish:package",
    "publish:string_name",
    "publish:target",
    "attr:feature",
    "scenario",
    "step:call",
    "step:async",
    "step:reply",
    "step:label",
    "step:parallel",
    "step:infra_hop",
    "allow",
];

/// Share of files that describe a system rather than a library.
const SYSTEM_SHARE: f64 = 0.8;

/// Writes `--count` samples that check without errors to `--out`, plus `report.json`.
pub(crate) fn gen_samples(args: &[String]) -> Result<(), String> {
    let mut out = None;
    let mut count = 1000_usize;
    let mut seed = 0_u64;
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        let value = it.next().ok_or(USAGE)?;
        match flag.as_str() {
            "--out" => out = Some(PathBuf::from(value)),
            "--count" => count = value.parse().map_err(|_| USAGE)?,
            "--seed" => seed = value.parse().map_err(|_| USAGE)?,
            _ => return Err(USAGE.to_owned()),
        }
    }
    let out = out.ok_or(USAGE)?;
    std::fs::create_dir_all(&out).map_err(|e| format!("can't create {}: {e}", out.display()))?;

    let mut report = Report::new(seed);
    for index in 0..count {
        let sample = generate(seed, index);
        if let Some(source) = report.add(&sample) {
            let path = out.join(format!("sample-{index:06}.saxl"));
            std::fs::write(&path, source)
                .map_err(|e| format!("can't write {}: {e}", path.display()))?;
        }
    }
    let path = out.join("report.json");
    let json = serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?;
    std::fs::write(&path, json + "\n")
        .map_err(|e| format!("can't write {}: {e}", path.display()))?;

    eprintln!(
        "wrote {} of {count} samples to {} ({} rejected)",
        report.written,
        out.display(),
        report.rejected
    );
    if !report.missing.is_empty() {
        eprintln!("constructs never emitted: {}", report.missing.join(", "));
    }
    Ok(())
}

/// What a run produced, written as `report.json`.
#[derive(serde::Serialize)]
pub(crate) struct Report {
    seed: u64,
    pub(crate) written: usize,
    pub(crate) rejected: usize,
    /// Error codes of rejected samples.
    pub(crate) rejected_codes: BTreeMap<&'static str, usize>,
    /// Warning and info codes of written samples.
    pub(crate) warning_codes: BTreeMap<&'static str, usize>,
    /// How many written samples contain each construct.
    coverage: BTreeMap<&'static str, usize>,
    /// Constructs no written sample contains.
    pub(crate) missing: Vec<&'static str>,
}

impl Report {
    fn new(seed: u64) -> Self {
        Self {
            seed,
            written: 0,
            rejected: 0,
            rejected_codes: BTreeMap::new(),
            warning_codes: BTreeMap::new(),
            coverage: CONSTRUCTS.iter().map(|c| (*c, 0)).collect(),
            missing: CONSTRUCTS.to_vec(),
        }
    }

    /// Checks a sample. Returns its canonical source if it has no errors.
    pub(crate) fn add(&mut self, sample: &Sample) -> Option<String> {
        let source = match saxl_parser::format(&sample.source) {
            Ok(source) => source,
            Err(diags) => {
                self.reject(diags.iter().map(|d| d.code.0));
                return None;
            }
        };
        let parsed = saxl_parser::parse(&source, FileId(0));
        let diags = saxl_validator::validate(parsed.model).diagnostics;
        let errors: Vec<_> = diags
            .iter()
            .filter(|d| d.severity == Severity::Error)
            .map(|d| d.code.0)
            .collect();
        if !errors.is_empty() {
            self.reject(errors.into_iter());
            return None;
        }
        self.written += 1;
        for d in &diags {
            *self.warning_codes.entry(d.code.0).or_default() += 1;
        }
        for tag in &sample.tags {
            *self.coverage.entry(tag).or_default() += 1;
        }
        self.missing.retain(|c| !sample.tags.contains(c));
        Some(source)
    }

    fn reject(&mut self, codes: impl Iterator<Item = &'static str>) {
        self.rejected += 1;
        for code in codes.collect::<BTreeSet<_>>() {
            *self.rejected_codes.entry(code).or_default() += 1;
        }
    }
}

/// One generated file, before formatting, and the constructs it contains.
pub(crate) struct Sample {
    pub(crate) source: String,
    pub(crate) tags: BTreeSet<&'static str>,
}

/// Generates sample `index` of run `seed`. The same pair always gives the same file.
pub(crate) fn generate(seed: u64, index: usize) -> Sample {
    let mut rng = Rng::new(seed ^ (index as u64).wrapping_mul(0xA24B_AED4_963E_E407));
    let mut out = Out::default();
    if rng.chance(SYSTEM_SHARE) {
        system(&mut rng, &mut out);
    } else {
        library(&mut rng, &mut out);
    }
    Sample {
        source: out.text,
        tags: out.tags,
    }
}

/// xorshift64*, as in the validator's robustness test, so runs need no dependency.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }

    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }

    /// A number in `lo..=hi`.
    fn range(&mut self, lo: usize, hi: usize) -> usize {
        lo + self.below(hi.saturating_sub(lo) + 1)
    }

    fn chance(&mut self, p: f64) -> bool {
        ((self.next() >> 11) as f64 / (1_u64 << 53) as f64) < p
    }

    fn pick<'a, T>(&mut self, xs: &'a [T]) -> &'a T {
        &xs[self.below(xs.len())]
    }

    fn shuffle<T>(&mut self, xs: &mut [T]) {
        for i in (1..xs.len()).rev() {
            xs.swap(i, self.below(i + 1));
        }
    }

    /// Between `lo` and `hi` distinct items, in random order.
    fn take_some<T: Clone>(&mut self, xs: &[T], lo: usize, hi: usize) -> Vec<T> {
        let n = self.range(lo, hi);
        self.take(xs, n)
    }

    /// Up to `n` distinct items, in random order.
    fn take<T: Clone>(&mut self, xs: &[T], n: usize) -> Vec<T> {
        let mut v = xs.to_vec();
        self.shuffle(&mut v);
        v.truncate(n);
        v
    }
}

/// The file being written and the constructs it contains so far.
#[derive(Default)]
struct Out {
    text: String,
    tags: BTreeSet<&'static str>,
}

impl Out {
    fn line(&mut self, line: &str) {
        self.text.push_str(line);
        self.text.push('\n');
    }

    fn blank(&mut self) {
        self.text.push('\n');
    }

    fn tag(&mut self, tag: &'static str) {
        self.tags.insert(tag);
    }

    /// `head "desc"` most of the time, `head` otherwise.
    fn described(&mut self, rng: &mut Rng, head: &str, desc: &str) -> String {
        if !rng.chance(0.8) {
            return head.to_owned();
        }
        self.tag("description");
        if !desc.is_ascii() {
            self.tag("unicode");
        }
        format!("{head} \"{desc}\"")
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Arrow {
    Sync,
    Async,
}

impl Arrow {
    fn text(self) -> &'static str {
        match self {
            Self::Sync => "->",
            Self::Async => "~>",
        }
    }
}

struct Rel {
    from: String,
    to: String,
    arrow: Arrow,
    desc: Option<&'static str>,
}

/// Relations plus each declared name's kind.
#[derive(Default)]
struct Graph {
    kinds: BTreeMap<String, &'static str>,
    rels: Vec<Rel>,
}

impl Graph {
    fn declare(&mut self, names: &[String], kind: &'static str) {
        for n in names {
            self.kinds.insert(n.clone(), kind);
        }
    }

    fn kind(&self, name: &str) -> &'static str {
        self.kinds.get(name).copied().unwrap_or("")
    }

    fn has(&self, from: &str, to: &str) -> bool {
        self.rels.iter().any(|r| r.from == from && r.to == to)
    }

    /// Adds `from -> to` unless it exists. Returns whether it was added.
    fn add(&mut self, from: &str, to: &str, arrow: Arrow) -> bool {
        if from == to || self.has(from, to) {
            return false;
        }
        self.rels.push(Rel {
            from: from.to_owned(),
            to: to.to_owned(),
            arrow,
            desc: None,
        });
        true
    }

    /// Adds `from -> to "desc"`.
    fn add_described(&mut self, from: &str, to: &str, desc: &'static str) {
        if self.add(from, to, Arrow::Sync)
            && let Some(r) = self.rels.last_mut()
        {
            r.desc = Some(desc);
        }
    }

    fn outgoing(&self, from: &str) -> Vec<(String, Arrow)> {
        self.rels
            .iter()
            .filter(|r| r.from == from)
            .map(|r| (r.to.clone(), r.arrow))
            .collect()
    }

    /// Writes the relations, joining some into chains such as `A -> B ~> C`.
    fn emit(&self, rng: &mut Rng, out: &mut Out) {
        let mut done = vec![false; self.rels.len()];
        for i in 0..self.rels.len() {
            if done[i] {
                continue;
            }
            done[i] = true;
            let first = &self.rels[i];
            let mut line = format!("{} {} {}", first.from, first.arrow.text(), first.to);
            let mut arrows = vec![first.arrow];
            if let Some(desc) = first.desc {
                // A chain can't carry a description (P004).
                line.push_str(&format!(" \"{desc}\""));
                out.tag("relation:description");
            } else if rng.chance(0.35) {
                let mut tail = first.to.clone();
                while arrows.len() < 3 {
                    let next = (0..self.rels.len()).find(|&j| {
                        !done[j] && self.rels[j].from == tail && self.rels[j].desc.is_none()
                    });
                    let Some(j) = next else { break };
                    done[j] = true;
                    let r = &self.rels[j];
                    line.push_str(&format!(" {} {}", r.arrow.text(), r.to));
                    arrows.push(r.arrow);
                    tail = r.to.clone();
                }
            }
            if arrows.len() > 1 {
                out.tag("relation:chain");
                if arrows.iter().any(|a| *a != arrows[0]) {
                    out.tag("relation:mixed_chain");
                }
            }
            for a in &arrows {
                out.tag(match a {
                    Arrow::Sync => "relation:sync",
                    Arrow::Async => "relation:async",
                });
            }
            out.line(&line);
        }
    }
}

fn names(pool: &[(&str, &str)]) -> Vec<String> {
    pool.iter().map(|(n, _)| (*n).to_owned()).collect()
}

fn desc_of(pool: &[(&'static str, &'static str)], name: &str) -> &'static str {
    pool.iter()
        .find(|(n, _)| *n == name)
        .map_or("", |(_, d)| *d)
}

/// Declares elements of one kind, one per line.
fn declare(
    rng: &mut Rng,
    out: &mut Out,
    kind: &'static str,
    tag: &'static str,
    list: &[String],
    pool: &[(&'static str, &'static str)],
) {
    for name in list {
        let line = out.described(rng, &format!("{kind} {name}"), desc_of(pool, name));
        out.line(&line);
        out.tag(tag);
    }
}

fn root(
    rng: &mut Rng,
    out: &mut Out,
    keyword: &str,
    pool: &[(&'static str, &'static str)],
) -> String {
    let (name, desc) = *rng.pick(pool);
    if rng.chance(0.4) {
        out.line(&format!("# {name}: {desc}"));
        out.tag("comment");
    }
    let line = out.described(rng, &format!("{keyword} {name}"), desc);
    out.line(&line);
    out.blank();
    name.to_owned()
}

/// `{ a, b }` on one line, or one field per line with a trailing comma.
fn fields(rng: &mut Rng, out: &mut Out, head: &str, pool: &[&str]) {
    let n = rng.range(1.min(pool.len()), pool.len());
    let list = &pool[..n];
    if list.len() > 1 && rng.chance(0.15) {
        out.tag("data:multiline");
        out.line(&format!("{head} {{"));
        for f in list {
            out.line(&format!("  {f},"));
        }
        out.line("}");
    } else {
        out.line(&format!("{head} {{ {} }}", list.join(", ")));
    }
}

/// `xN`, `autoscale`, or nothing, for a compute workload.
fn compute_attrs(rng: &mut Rng, out: &mut Out) -> String {
    let mut attrs = Vec::new();
    if rng.chance(0.6) {
        attrs.push(format!("x{}", rng.range(1, 5)));
        out.tag("attr:replicas");
    }
    if rng.chance(0.25) {
        attrs.push("autoscale".to_owned());
        out.tag("attr:autoscale");
    }
    attrs.join(" ")
}

/// Splits names into workload lines of one to `max` names, such as `A, B x2`.
fn workload_lines(
    rng: &mut Rng,
    out: &mut Out,
    names: &[String],
    max: usize,
    attrs: &mut dyn FnMut(&mut Rng, &mut Out, &[String]) -> String,
) -> Vec<String> {
    let mut lines = Vec::new();
    let mut rest = names;
    while !rest.is_empty() {
        let n = rng.range(1, max.min(rest.len()));
        let (chunk, tail) = rest.split_at(n);
        rest = tail;
        if chunk.len() > 1 {
            out.tag("workload:list");
        }
        let a = attrs(rng, out, chunk);
        let line = chunk.join(", ");
        lines.push(if a.is_empty() {
            line
        } else {
            format!("{line} {a}")
        });
    }
    lines
}

// Systems.

fn system(rng: &mut Rng, out: &mut Out) {
    out.tag("root:system");
    root(rng, out, "system", SYSTEMS);

    let actors = rng.take_some(&names(ACTORS), 1, 3);
    let externals = rng.take_some(&names(EXTERNALS), 0, 3);
    let services = rng.take_some(&names(SERVICES), 1, 8);
    let stores = rng.take_some(&names(STORES), 1, 4);
    let queues = if services.len() >= 2 {
        rng.take_some(&names(QUEUES), 0, 2)
    } else {
        Vec::new()
    };

    declare(rng, out, "actor", "element:actor", &actors, ACTORS);
    declare(
        rng,
        out,
        "external",
        "element:external",
        &externals,
        EXTERNALS,
    );
    out.blank();
    declare(rng, out, "service", "element:service", &services, SERVICES);
    declare(rng, out, "store", "element:store", &stores, STORES);
    declare(rng, out, "queue", "element:queue", &queues, QUEUES);
    out.blank();

    let mut g = Graph::default();
    g.declare(&actors, "actor");
    g.declare(&externals, "external");
    g.declare(&services, "service");
    g.declare(&stores, "store");
    g.declare(&queues, "queue");

    // Actors call the entry services. Other services are called by an earlier
    // one, so synchronous calls between services always point forward: no cycles.
    // Each entry service gets at least one actor, or nothing would call it.
    let entries = rng.range(1, services.len().min(actors.len()).min(2));
    for (i, a) in actors.iter().enumerate() {
        let entry = services[if i < entries { i } else { rng.below(entries) }].clone();
        if rng.chance(0.3) {
            g.add_described(a, &entry, rng.pick(ACTOR_REL_DESC));
        } else {
            g.add(a, &entry, Arrow::Sync);
        }
    }
    for i in 1..services.len() {
        if i >= entries {
            g.add(&services[rng.below(i)], &services[i], Arrow::Sync);
        }
        if rng.chance(0.2) {
            g.add(&services[rng.below(i)], &services[i], Arrow::Sync);
        }
    }
    for s in &stores {
        g.add(rng.pick(&services), s, Arrow::Sync);
        if rng.chance(0.25) {
            g.add(rng.pick(&services), s, Arrow::Sync);
        }
    }
    for e in &externals {
        // Either the system calls the external or the external calls in, never
        // both, so an external can't close a cycle.
        if rng.chance(0.2) {
            g.add_described(e, &services[rng.below(entries)], "webhooks");
        } else {
            g.add(rng.pick(&services), e, Arrow::Sync);
        }
    }
    for q in &queues {
        // A producer and a different consumer, so W106 never fires.
        let p = rng.below(services.len());
        let c = (p + rng.range(1, services.len() - 1)) % services.len();
        g.add(&services[p], q, Arrow::Async);
        g.add(q, &services[c], Arrow::Async);
    }
    g.emit(rng, out);

    let data: Vec<_> = rng.take_some(ENTITIES, 0, 4);
    if !data.is_empty() {
        out.blank();
    }
    for (name, pool) in &data {
        let store = rng.pick(&stores).clone();
        let head = if !queues.is_empty() && rng.chance(0.25) {
            out.tag("data:via");
            format!("data {name} via {} -> {store}", rng.pick(&queues))
        } else {
            out.tag("data:in");
            let others: Vec<_> = stores.iter().filter(|s| **s != store).cloned().collect();
            if !others.is_empty() && rng.chance(0.3) {
                out.tag("data:cached");
                let cached = rng.take_some(&others, 1, 2);
                format!("data {name} in {store} cached {}", cached.join(", "))
            } else {
                format!("data {name} in {store}")
            }
        };
        fields(rng, out, &head, pool);
    }

    let mut cdn = None;
    let mut single_region = false;
    if rng.chance(0.8) {
        out.tag("deploy");
        let n = if rng.chance(0.3) { 2 } else { 1 };
        let regions = rng.take(REGIONS, n);
        single_region = regions.len() == 1;
        if !single_region {
            out.tag("deploy:multi_region");
        }
        if rng.chance(0.35) {
            cdn = Some(*rng.pick(CDNS));
        }
        let clusters = rng.take(CLUSTERS, 2);
        for (i, region) in regions.iter().enumerate() {
            out.blank();
            out.line(&format!("deploy region {region} {{"));
            if let (0, Some(name)) = (i, cdn) {
                out.tag("deploy:cdn");
                out.line(&format!("  cdn {name}"));
            }
            let split = if services.len() >= 3 && rng.chance(0.3) {
                rng.range(1, services.len() - 1)
            } else {
                services.len()
            };
            for (cluster, group) in clusters
                .iter()
                .zip([&services[..split], &services[split..]])
            {
                if group.is_empty() {
                    continue;
                }
                out.tag("deploy:cluster");
                out.line(&format!("  cluster {cluster} {{"));
                for line in workload_lines(rng, out, group, 2, &mut |rng, out, _| {
                    compute_attrs(rng, out)
                }) {
                    out.line(&format!("    {line}"));
                }
                out.line("  }");
            }
            out.tag("deploy:managed");
            out.line("  managed {");
            let managed: Vec<_> = stores.iter().chain(&queues).cloned().collect();
            for line in workload_lines(rng, out, &managed, 3, &mut |rng, out, chunk| {
                let all_stores = chunk.iter().all(|n| stores.contains(n));
                if all_stores && rng.chance(0.5) {
                    out.tag("attr:primary_replica");
                    "primary+replica".to_owned()
                } else {
                    String::new()
                }
            }) {
                out.line(&format!("    {line}"));
            }
            out.line("  }");
            out.line("}");
        }
    }

    if rng.chance(0.5) {
        scenarios(
            rng,
            out,
            &g,
            &actors,
            cdn,
            SCENARIOS,
            CALL_LABELS,
            REPLY_LABELS,
        );
    }

    if single_region && rng.chance(0.3) {
        out.tag("allow");
        out.blank();
        out.line("allow W102 \"a single region is acceptable for now\"");
    }
}

// Libraries.

fn library(rng: &mut Rng, out: &mut Out) {
    out.tag("root:library");
    let lib = root(rng, out, "library", LIBRARIES).to_lowercase();

    let consumers = rng.take_some(&names(CONSUMERS), 1, 2);
    let apis = rng.take_some(&names(APIS), 1, 3);
    let modules = rng.take_some(&names(MODULES), 1, 5);
    let mut packages = rng.take_some(&names(PACKAGES), 0, 3);
    let mut hooks = rng.take_some(&names(HOOKS), 0, 2);

    // A feature-gated plugin module: it depends on a package and implements a
    // hook, and nothing depends on it, so gating it can't trigger W109.
    let plugin = rng.chance(0.3).then(|| *rng.pick(PLUGINS));
    if let Some((_, _, package)) = plugin {
        if !packages.iter().any(|p| p == package) {
            packages.push(package.to_owned());
        }
        if hooks.is_empty() {
            hooks.push(rng.pick(HOOKS).0.to_owned());
        }
    }
    let mut code = modules.clone();
    code.extend(plugin.map(|(name, _, _)| name.to_owned()));

    let at = rng.chance(0.5);
    if at {
        out.tag("at");
    }
    let with_at = |rng: &mut Rng, line: String, name: &str| {
        if !at {
            return line;
        }
        let path = if rng.chance(0.2) {
            format!("src/{name}/**")
        } else {
            format!("src/{name}.rs")
        };
        format!("{line} at \"{path}\"")
    };

    declare(
        rng,
        out,
        "consumer",
        "element:consumer",
        &consumers,
        CONSUMERS,
    );
    let package_pool: Vec<_> = PACKAGES.iter().chain(PLUGIN_PACKAGES).copied().collect();
    declare(
        rng,
        out,
        "package",
        "element:package",
        &packages,
        &package_pool,
    );
    out.blank();
    for name in &apis {
        out.tag("element:api");
        let line = out.described(rng, &format!("api {name}"), desc_of(APIS, name));
        let line = with_at(rng, line, name);
        out.line(&line);
    }
    for name in &code {
        out.tag("element:module");
        let desc = plugin
            .filter(|(p, _, _)| p == name)
            .map_or_else(|| desc_of(MODULES, name), |(_, d, _)| d);
        let line = out.described(rng, &format!("module {name}"), desc);
        let line = with_at(rng, line, name);
        out.line(&line);
    }
    declare(rng, out, "hook", "element:hook", &hooks, HOOKS);
    out.blank();

    let mut g = Graph::default();
    g.declare(&consumers, "consumer");
    g.declare(&packages, "package");
    g.declare(&apis, "api");
    g.declare(&code, "module");
    g.declare(&hooks, "hook");

    // Consumers call only apis (E012). Apis call modules, and each module is
    // called by an api or an earlier module: no synchronous cycles.
    for c in &consumers {
        let arrow = if rng.chance(0.2) {
            Arrow::Async
        } else {
            Arrow::Sync
        };
        g.add(c, rng.pick(&apis), arrow);
    }
    for (i, m) in modules.iter().enumerate() {
        let caller = if i == 0 || rng.chance(0.4) {
            rng.pick(&apis).clone()
        } else {
            modules[rng.below(i)].clone()
        };
        g.add(&caller, m, Arrow::Sync);
    }
    for a in &apis {
        if g.outgoing(a).is_empty() {
            g.add(a, rng.pick(&modules), Arrow::Sync);
        }
    }
    let plugin_package = plugin.map(|(_, _, p)| p);
    for p in &packages {
        if Some(p.as_str()) == plugin_package {
            continue;
        }
        g.add(rng.pick(&modules), p, Arrow::Sync);
    }
    if let Some((name, _, package)) = plugin {
        g.add(name, package, Arrow::Sync);
    }
    for h in &hooks {
        g.add(rng.pick(&modules), h, Arrow::Sync);
    }
    g.emit(rng, out);

    // `A impl H`: the plugin implements the first hook; others sometimes get an
    // implementation from a module or a consumer.
    // I202 reports a hook no api or module implements.
    let mut needs_allow = false;
    for (i, h) in hooks.iter().enumerate() {
        // (implementor, whether it is the library's own code)
        let implementor = match plugin {
            Some((name, _, _)) if i == 0 => Some((name.to_owned(), true)),
            _ if rng.chance(0.5) => Some((rng.pick(&modules).clone(), true)),
            _ if rng.chance(0.4) => Some((rng.pick(&consumers).clone(), false)),
            _ => None,
        };
        if let Some((name, _)) = &implementor {
            out.tag("impl");
            out.line(&format!("{name} impl {h}"));
        }
        needs_allow |= !implementor.is_some_and(|(_, internal)| internal);
    }

    let types: Vec<_> = rng.take_some(TYPES, 0, 3);
    if !types.is_empty() {
        out.blank();
    }
    let owners: Vec<_> = apis.iter().chain(&modules).cloned().collect();
    for (name, pool) in &types {
        out.tag("data:in");
        let mut head = format!("data {name} in {}", rng.pick(&owners));
        if rng.chance(0.5) {
            out.tag("data:pub");
            head.push_str(" pub");
        }
        fields(rng, out, &head, pool);
    }

    if rng.chance(0.85) {
        publish(rng, out, &lib, &apis, &modules, plugin);
    }

    if rng.chance(0.5) {
        scenarios(
            rng,
            out,
            &g,
            &consumers,
            None,
            LIB_SCENARIOS,
            LIB_CALL_LABELS,
            LIB_REPLY_LABELS,
        );
    }

    // Hooks left to consumers are often intended, as with plugin hooks.
    if needs_allow && rng.chance(0.6) {
        out.tag("allow");
        out.blank();
        out.line("allow I202 \"implementations are opt-in\"");
    }
}

fn publish(
    rng: &mut Rng,
    out: &mut Out,
    lib: &str,
    apis: &[String],
    modules: &[String],
    plugin: Option<(&str, &str, &str)>,
) {
    out.tag("publish");
    let code: Vec<_> = apis.iter().chain(modules).cloned().collect();
    let registry = rng.below(100);
    out.blank();
    let (header, kind, package) = if registry < 60 {
        out.tag("publish:string_name");
        ("publish \"crates.io\" {", "crate", lib.to_owned())
    } else if registry < 85 {
        out.tag("publish:string_name");
        ("publish npm {", "package", format!("\"@{lib}/core\""))
    } else {
        ("publish pypi {", "package", lib.to_owned())
    };
    out.tag(if kind == "crate" {
        "publish:crate"
    } else {
        "publish:package"
    });
    out.line(header);
    out.line(&format!("  {kind} {package} {{"));
    for line in workload_lines(rng, out, &code, 4, &mut |_, _, _| String::new()) {
        out.line(&format!("    {line}"));
    }
    if let Some((name, _, feature)) = plugin {
        out.tag("attr:feature");
        out.line(&format!("    {name} feature {feature}"));
    }
    out.line("  }");
    out.line("}");

    if rng.chance(0.25) {
        out.tag("publish:package");
        out.tag("publish:string_name");
        out.tag("publish:target");
        out.blank();
        out.line("publish npm {");
        out.line(&format!("  package \"@{lib}/wasm\" target wasm32 {{"));
        out.line(&format!("    {}", apis.join(", ")));
        out.line("  }");
        out.line("}");
    }
}

// Scenarios.

/// One or two scenarios, each starting from an outside caller.
#[allow(clippy::too_many_arguments)]
fn scenarios(
    rng: &mut Rng,
    out: &mut Out,
    g: &Graph,
    callers: &[String],
    cdn: Option<&str>,
    names: &[&str],
    calls: &[&'static str],
    replies: &[&'static str],
) {
    // Scenario starts: an outside caller's synchronous call into the system.
    let starts: Vec<_> = g
        .rels
        .iter()
        .filter(|r| callers.contains(&r.from) && r.arrow == Arrow::Sync)
        .collect();
    if starts.is_empty() {
        return;
    }
    for name in rng.take_some(names, 1, 2) {
        let start = *rng.pick(&starts);
        let mut walk = Walk {
            g,
            steps: Vec::new(),
            budget: 12,
            calls,
            replies,
            tags: BTreeSet::new(),
        };
        let (caller, entry) = (start.from.as_str(), start.to.as_str());
        match cdn {
            Some(edge) if rng.chance(0.5) => {
                walk.tags.insert("step:infra_hop");
                walk.steps
                    .push(Step::Line(format!("{caller} -> {edge} -> {entry}")));
            }
            _ => walk.call(rng, caller, entry),
        }
        // Sometimes the final reply and one message are sent in parallel.
        let deferred = g
            .outgoing(entry)
            .into_iter()
            .find(|(_, a)| *a == Arrow::Async)
            .filter(|_| rng.chance(0.4));
        walk.visit(rng, entry, 0, deferred.as_ref().map(|(to, _)| to.as_str()));
        let reply = walk.reply_line(rng, entry, caller);
        match deferred {
            Some((to, _)) => {
                walk.tags.insert("step:parallel");
                walk.tags.insert("step:async");
                walk.steps
                    .push(Step::Parallel(vec![reply, format!("{entry} ~> {to}")]));
            }
            None => walk.steps.push(Step::Line(reply)),
        }

        out.tag("scenario");
        out.tags.extend(walk.tags);
        out.blank();
        out.line(&format!("scenario {name} {{"));
        for step in walk.steps {
            match step {
                Step::Line(line) => out.line(&format!("  {line}")),
                Step::Parallel(lines) => {
                    out.line("  parallel {");
                    for line in lines {
                        out.line(&format!("    {line}"));
                    }
                    out.line("  }");
                }
            }
        }
        out.line("}");
    }
}

enum Step {
    Line(String),
    Parallel(Vec<String>),
}

/// Walks synchronous calls depth first, replying to each call on the way back,
/// so every step follows a relation (E006) and every reply closes a call (E007).
struct Walk<'a> {
    g: &'a Graph,
    steps: Vec<Step>,
    budget: usize,
    calls: &'a [&'static str],
    replies: &'a [&'static str],
    tags: BTreeSet<&'static str>,
}

impl Walk<'_> {
    fn label(&mut self, rng: &mut Rng, pool: &[&'static str]) -> String {
        if rng.chance(0.6) {
            self.tags.insert("step:label");
            format!(" : {}", rng.pick(pool))
        } else {
            String::new()
        }
    }

    fn call(&mut self, rng: &mut Rng, from: &str, to: &str) {
        self.tags.insert("step:call");
        let label = self.label(rng, self.calls);
        self.steps
            .push(Step::Line(format!("{from} -> {to}{label}")));
    }

    fn reply_line(&mut self, rng: &mut Rng, from: &str, to: &str) -> String {
        self.tags.insert("step:reply");
        let label = self.label(rng, self.replies);
        format!("{from} --> {to}{label}")
    }

    fn visit(&mut self, rng: &mut Rng, node: &str, depth: usize, skip: Option<&str>) {
        let mut next = self.g.outgoing(node);
        next.retain(|(to, _)| Some(to.as_str()) != skip);
        rng.shuffle(&mut next);
        next.truncate(if depth == 0 { 3 } else { 2 });
        for (to, arrow) in next {
            if self.budget == 0 {
                return;
            }
            self.budget -= 1;
            match arrow {
                Arrow::Sync => {
                    self.call(rng, node, &to);
                    let descend = matches!(self.g.kind(&to), "service" | "api" | "module");
                    if descend && depth < 2 {
                        self.visit(rng, &to, depth + 1, None);
                    }
                    let reply = self.reply_line(rng, &to, node);
                    self.steps.push(Step::Line(reply));
                }
                Arrow::Async => {
                    self.tags.insert("step:async");
                    self.steps.push(Step::Line(format!("{node} ~> {to}")));
                    // Follow the message through its queue.
                    if self.g.kind(&to) == "queue"
                        && rng.chance(0.5)
                        && let Some((consumer, _)) = self.g.outgoing(&to).first()
                    {
                        self.steps.push(Step::Line(format!("{to} ~> {consumer}")));
                    }
                }
            }
        }
    }
}

// Name pools. Pools for one root never share a name, so E001 can't fire.

const SYSTEMS: &[(&str, &str)] = &[
    ("Parcel", "package tracking"),
    ("Beacon", "status pages"),
    ("Harbor", "container registry"),
    ("Pulse", "IoT telemetry"),
    ("Courier", "food delivery"),
    ("Agora", "discussion forum"),
    ("Turnstile", "event ticketing"),
    ("Shelf", "online bookstore"),
    ("Relay", "team chat"),
    ("Ledgerly", "invoicing"),
    ("Clinic", "appointment booking"),
    ("Fleet", "ride hailing"),
];

const ACTORS: &[(&str, &str)] = &[
    ("Customer", "places orders"),
    ("Admin", "manages settings"),
    ("Visitor", "browses anonymously"),
    ("Merchant", "sells products"),
    ("Driver", "delivers orders"),
    ("Operator", "watches dashboards"),
    ("Author", "writes posts"),
    ("Reader", "reads posts"),
    ("Agent", "answers tickets"),
    ("Patient", "books visits"),
];

const EXTERNALS: &[(&str, &str)] = &[
    ("Stripe", "card payments"),
    ("Twilio", "SMS"),
    ("SendGrid", "email delivery"),
    ("Auth0", "sign-in"),
    ("Maps", "geocoding"),
    ("Slack", "notifications"),
    ("GitHub", "OAuth and webhooks"),
    ("Shippo", "shipping labels"),
    ("Sentry", "error reports"),
];

const SERVICES: &[(&str, &str)] = &[
    ("Web", "web frontend"),
    ("Api", "public API"),
    ("Gateway", "auth, routing"),
    ("Orders", "order lifecycle"),
    ("Billing", "invoices and charges"),
    ("Accounts", "users and teams"),
    ("Catalog", "products"),
    ("Search", "full-text queries"),
    ("Notifier", "email and push"),
    ("Worker", "background jobs"),
    ("Scheduler", "cron jobs"),
    ("Ingest", "device → stream"),
    ("Reports", "exports and analytics"),
    ("Auth", "sessions and tokens"),
    ("Inventory", "stock levels"),
    ("Checkout", "orchestrates payment"),
    ("Media", "image processing"),
    ("Feed", "timeline assembly"),
];

const STORES: &[(&str, &str)] = &[
    ("Postgres", "source of truth"),
    ("MySQL", "relational data"),
    ("Redis", "key → value cache"),
    ("Mongo", "documents"),
    ("Elastic", "search index"),
    ("Blob", "object storage"),
    ("Warehouse", "analytics"),
    ("Cassandra", "time series"),
    ("Dynamo", "key-value"),
];

const QUEUES: &[(&str, &str)] = &[
    ("Kafka", "event stream"),
    ("Events", "domain events"),
    ("Jobs", "work queue"),
    ("Outbox", "pending messages"),
    ("Tasks", "deferred tasks"),
    ("Clickstream", "raw clicks"),
];

const CDNS: &[&str] = &["Edge", "Front", "Cloudfront"];

const REGIONS: &[&str] = &[
    "us-east-1",
    "us-west-2",
    "eu-west-1",
    "eu-central-1",
    "ap-southeast-1",
];

const CLUSTERS: &[&str] = &["k8s", "main", "apps", "core", "workers"];

const ENTITIES: &[(&str, &[&str])] = &[
    ("User", &["id", "email", "name", "created_at"]),
    ("Order", &["id", "user_id", "total", "status"]),
    ("Invoice", &["id", "order_id", "amount", "due_at"]),
    ("Product", &["sku", "title", "price"]),
    ("Session", &["token", "user_id", "expires_at"]),
    ("Event", &["id", "kind", "payload", "ts"]),
    ("Payment", &["id", "invoice_id", "provider_ref", "state"]),
    ("Message", &["id", "channel", "body", "sent_at"]),
    ("Reading", &["device_id", "metric", "value", "ts"]),
    ("Ticket", &["id", "event_id", "seat", "holder"]),
];

const ACTOR_REL_DESC: &[&str] = &[
    "browses",
    "signs in",
    "places orders",
    "GET /status",
    "uses the app",
];

const SCENARIOS: &[&str] = &[
    "place_order",
    "sign_in",
    "checkout",
    "send_message",
    "upload_image",
    "view_dashboard",
    "refund",
    "search_items",
    "sign_up",
    "track_parcel",
];

const CALL_LABELS: &[&str] = &[
    "GET /items",
    "POST /orders",
    "lookup",
    "load session",
    "charge card",
    "insert row",
    "fetch profile",
    "validate token",
    "render page",
];

const REPLY_LABELS: &[&str] = &["200", "ok", "rows", "token", "not found", "html", "receipt"];

const LIBRARIES: &[(&str, &str)] = &[
    ("Ferry", "HTTP client"),
    ("Prism", "syntax highlighting"),
    ("Tally", "metrics collection"),
    ("Sieve", "input validation"),
    ("Atlas", "geocoding SDK"),
    ("Loom", "async runtime"),
    ("Inkwell", "terminal UI"),
    ("Quarry", "SQL query builder"),
    ("Parchment", "PDF rendering"),
];

const CONSUMERS: &[(&str, &str)] = &[
    ("Cli", "command line"),
    ("WebApp", "browser app"),
    ("Server", "backend service"),
    ("Editor", "live preview"),
    ("Bot", "chat bot"),
    ("Notebook", "data notebooks"),
];

const APIS: &[(&str, &str)] = &[
    ("parse", "text → tree"),
    ("render", "tree → output"),
    ("encode", "value → bytes"),
    ("decode", "bytes → value"),
    ("connect", "opens a session"),
    ("query", "runs a query"),
    ("compile", "source → program"),
    ("validate", "checks input"),
    ("load", "reads config"),
    ("send", "issues a request"),
];

const MODULES: &[(&str, &str)] = &[
    ("lexer", "tokens"),
    ("ast", "syntax tree"),
    ("cache", "memoized results"),
    ("pool", "connection pool"),
    ("codec", "wire format"),
    ("config", "settings"),
    ("retry", "backoff"),
    ("schema", "type rules"),
    ("writer", "output buffer"),
    ("reader", "input buffer"),
    ("router", "dispatch"),
    ("escape", "entities"),
    ("emitter", "code output"),
    ("planner", "query plans"),
];

/// (module, description, the package it wraps and the feature that gates it).
const PLUGINS: &[(&str, &str, &str)] = &[
    ("highlight", "syntax colors", "syntect"),
    ("compress", "gzip bodies", "flate2"),
    ("tls", "encrypted transport", "rustls"),
    ("telemetry", "traces", "opentelemetry"),
];

const PACKAGES: &[(&str, &str)] = &[
    ("serde", "serialization"),
    ("regex", "pattern matching"),
    ("tokio", "async IO"),
    ("unicode", "segmentation"),
    ("chrono", "dates"),
    ("libc", "system calls"),
];

const PLUGIN_PACKAGES: &[(&str, &str)] = &[
    ("syntect", "highlighting"),
    ("flate2", "compression"),
    ("rustls", "TLS"),
    ("opentelemetry", "tracing"),
];

const HOOKS: &[(&str, &str)] = &[
    ("Logger", "log sink"),
    ("Transport", "sends bytes"),
    ("Storage", "persists state"),
    ("Highlighter", "colors code"),
    ("Resolver", "finds names"),
    ("Clock", "current time"),
    ("Authenticator", "credentials"),
];

const TYPES: &[(&str, &[&str])] = &[
    ("Doc", &["blocks", "meta"]),
    ("Token", &["kind", "text", "line"]),
    ("Options", &["timeout", "retries", "verbose"]),
    ("Request", &["method", "url", "headers", "body"]),
    ("Response", &["status", "headers", "body"]),
    ("Row", &["columns", "values"]),
    ("Plan", &["steps", "cost"]),
];

const LIB_SCENARIOS: &[&str] = &["render_page", "send_request", "run_query", "load_config"];

const LIB_CALL_LABELS: &[&str] = &["src", "bytes", "tokens", "query", "options"];

const LIB_REPLY_LABELS: &[&str] = &["Doc", "Ok", "Err", "String", "rows"];

#[cfg(test)]
#[cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]
mod tests {
    use super::*;

    #[test]
    fn samples_check_and_cover_every_construct() {
        let mut report = Report::new(7);
        for index in 0..2000 {
            report.add(&generate(7, index));
        }
        assert_eq!(report.rejected, 0, "{:?}", report.rejected_codes);
        // Warnings that mean the generator made a mistake rather than a choice.
        let mistakes: Vec<_> = report
            .warning_codes
            .keys()
            .filter(|c| ["W104", "W105", "W106", "W107", "W108", "W109"].contains(c))
            .collect();
        assert!(mistakes.is_empty(), "{mistakes:?}");
        assert!(
            report.missing.is_empty(),
            "never emitted: {:?}",
            report.missing
        );
    }

    #[test]
    fn same_seed_same_file() {
        assert_eq!(generate(3, 41).source, generate(3, 41).source);
        assert_ne!(generate(3, 41).source, generate(4, 41).source);
    }
}
