//! Data view: stores (and queues) in a row on top, entity cards below in a
//! grid of up to three columns, in declaration order.

use crate::layout::{PAD, node_size, place};
use crate::project::Data;
use crate::scene::{Point2, Rect, SceneEdge, SceneNode, Size2};

const COLUMNS: usize = 3;
const GAP: f64 = 40.0;
/// Space between the store row and the cards, for edges.
const ROW_GAP: f64 = 80.0;

pub(crate) struct Layout {
    pub nodes: Vec<SceneNode>,
    pub edges: Vec<SceneEdge>,
    pub size: Size2,
}

pub(crate) fn layout(p: Data) -> Layout {
    let mut nodes = Vec::new();
    let mut x = PAD;
    let mut store_h: f64 = 0.0;
    for s in p.stores {
        let (w, h) = node_size(&s);
        nodes.push(place(s, Rect::new(x, PAD, w, h)));
        x += w + GAP;
        store_h = store_h.max(h);
    }
    let stores_right = x - GAP;
    let store_count = nodes.len();

    let sizes: Vec<(f64, f64)> = p.cards.iter().map(node_size).collect();
    let col_w = sizes.iter().map(|s| s.0).fold(0.0, f64::max);
    let mut y = PAD + store_h + ROW_GAP;
    let mut bottom = PAD + store_h;
    for (row, chunk) in p.cards.chunks(COLUMNS).enumerate() {
        let row_h = sizes[row * COLUMNS..row * COLUMNS + chunk.len()]
            .iter()
            .map(|s| s.1)
            .fold(0.0, f64::max);
        for (col, card) in chunk.iter().enumerate() {
            let (w, h) = sizes[row * COLUMNS + col];
            let rect = Rect::new(PAD + (col_w + GAP) * col as f64, y, w.max(col_w), h);
            nodes.push(place(card.clone(), rect));
        }
        bottom = y + row_h;
        y += row_h + GAP;
    }

    // Orthogonal routes: up from the card, along a channel under the stores
    // (one track per edge), then up into the store. Cards below the first row
    // first climb through the gap to their right, which no card occupies.
    let channel_top = PAD + store_h + 16.0;
    let mut card_ports = vec![0usize; p.cards.len()];
    let mut store_ports: Vec<Vec<(f64, usize)>> = vec![Vec::new(); store_count];
    for (e, (card, store, _, _)) in p.links.iter().enumerate() {
        store_ports[*store].push((nodes[store_count + card].rect.x, e));
    }
    let mut store_x = vec![0.0; p.links.len()];
    for (s, ports) in store_ports.iter_mut().enumerate() {
        ports.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        let k = ports.len() as f64;
        for (i, &(_, e)) in ports.iter().enumerate() {
            store_x[e] = nodes[s].rect.center().x + (i as f64 - (k - 1.0) / 2.0) * 16.0;
        }
    }
    let first_row_y = PAD + store_h + ROW_GAP;
    let mut edges = Vec::new();
    for (e, &(card, store, style, _)) in p.links.iter().enumerate() {
        let (c, s) = (&nodes[store_count + card], &nodes[store]);
        let j = card_ports[card];
        card_ports[card] += 1;
        let px = c.rect.x + 24.0 + 16.0 * j as f64;
        let track = channel_top + 10.0 * (e % 6) as f64;
        let mut points = vec![Point2::new(px, c.rect.y)];
        if c.rect.y > first_row_y {
            let lane_y = c.rect.y - GAP / 2.0 + 4.0 * j as f64;
            let lane_x = c.rect.right() + GAP / 2.0 + 4.0 * ((e % 5) as f64 - 2.0);
            points.push(Point2::new(px, lane_y));
            points.push(Point2::new(lane_x, lane_y));
            points.push(Point2::new(lane_x, track));
        } else {
            points.push(Point2::new(px, track));
        }
        points.push(Point2::new(store_x[e], track));
        points.push(Point2::new(store_x[e], s.rect.bottom()));
        edges.push(SceneEdge {
            from: c.id.clone(),
            to: s.id.clone(),
            style,
            points,
            label: None,
        });
    }

    let cards = p.cards.len().min(COLUMNS) as f64;
    let cards_right = PAD + cards * col_w + (cards - 1.0).max(0.0) * GAP;
    let lanes = if p.cards.len() > COLUMNS {
        GAP / 2.0 + 8.0
    } else {
        0.0
    };
    Layout {
        nodes,
        edges,
        size: Size2 {
            w: stores_right.max(cards_right + lanes) + PAD,
            h: bottom + PAD,
        },
    }
}
