import './style.css';
import { mountCarousels, type RailInput } from 'tv-ui-web';

/** A handful of rails/cards — the host page owns its own content and shape. */
function buildRails(specs: { title: string; count: number; seed: string }[], nextId: () => number): RailInput[] {
  return specs.map(({ title, count, seed }) => ({
    title,
    cards: Array.from({ length: count }, (_, i) => ({
      id: nextId(),
      title: `${title} ${i + 1}`,
      imageUrl: `https://picsum.photos/seed/${seed}-${i}/200/300`,
    })),
  }));
}

let idCounter = 0;
const nextId = () => idCounter++;

const initialRails = buildRails(
  [
    { title: 'Featured', count: 6, seed: 'embed-featured' },
    { title: 'Continue Watching', count: 5, seed: 'embed-continue' },
    { title: 'New This Week', count: 7, seed: 'embed-new' },
  ],
  nextId,
);

const canvas = document.querySelector<HTMLCanvasElement>('#carousel')!;
const statusEl = document.querySelector<HTMLElement>('#status')!;
const detailEl = document.querySelector<HTMLElement>('#detail')!;
const loadStatusEl = document.querySelector<HTMLElement>('#load-status')!;

// Plain typed objects in — no JSON.stringify.
const handle = mountCarousels(canvas, initialRails);

// --- Auto-load more rails as focus nears the end, driven entirely by the
// `focuschange` event — no polling, no manual "load more" action. ---------

const AUTO_LOAD_MAX_BATCHES = 3;
/** Trigger a load once focus is within this many rails of the last one. */
const NEAR_END_THRESHOLD = 1;

let totalRails = initialRails.length;
let batchesLoaded = 0;

function maybeLoadMore(focusedRailIndex: number) {
  if (batchesLoaded >= AUTO_LOAD_MAX_BATCHES) return;
  if (focusedRailIndex + NEAR_END_THRESHOLD < totalRails - 1) return;

  batchesLoaded += 1;
  const more = buildRails(
    [{ title: `More Like This ${batchesLoaded}`, count: 6, seed: `embed-more-${batchesLoaded}` }],
    nextId,
  );
  // appendRails adds to the end without resetting focus/scroll/animations —
  // this fires reactively from focus movement, not a manual click.
  handle.appendRails(more);
  totalRails += more.length;

  loadStatusEl.textContent =
    batchesLoaded >= AUTO_LOAD_MAX_BATCHES
      ? `Loaded ${batchesLoaded}/${AUTO_LOAD_MAX_BATCHES} auto-batches — all caught up.`
      : `Loaded ${batchesLoaded}/${AUTO_LOAD_MAX_BATCHES} auto-batches — keep scrolling down for more.`;
}

// `eventTarget` is a standard EventTarget: addEventListener/removeEventListener,
// any number of listeners, no bespoke callback API.
handle.eventTarget.addEventListener('focuschange', (e) => {
  const { railIndex, cardIndex } = (e as CustomEvent).detail;
  statusEl.textContent = `Focused: rail ${railIndex}, card ${cardIndex}`;
  maybeLoadMore(railIndex);
});

handle.eventTarget.addEventListener('select', (e) => {
  const { cardTitle, cardId } = (e as CustomEvent).detail;
  detailEl.textContent = `Selected "${cardTitle}" (id ${cardId}) — the driving app would open a detail page here.`;
});

// Demonstrate teardown: unmount if the canvas is ever removed from the page.
window.addEventListener('beforeunload', () => handle.unmount());
