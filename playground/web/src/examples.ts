// The examples menu. Sources come straight from the repository.

import orders from "../../../tests/fixtures/valid/orders.saxl?raw";
import shopfront from "../../../examples/shopfront.saxl?raw";
import snip from "../../../examples/snip.saxl?raw";

export const EXAMPLES: { name: string; source: string }[] = [
  { name: "Snip (URL shortener)", source: snip },
  { name: "Shopfront (online store)", source: shopfront },
  { name: "Orders", source: orders },
];
