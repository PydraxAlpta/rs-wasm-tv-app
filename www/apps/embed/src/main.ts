import './style.css';
import { mountCarousels } from 'tv-ui-web';

/** A handful of rails/cards — the host page owns its own content and shape. */
function sampleData() {
  const rails = [
    { title: 'Featured', count: 6, seed: 'embed-featured' },
    { title: 'Continue Watching', count: 5, seed: 'embed-continue' },
    { title: 'New This Week', count: 7, seed: 'embed-new' },
  ];

  let nextId = 0;
  return {
    rails: rails.map(({ title, count, seed }) => ({
      title,
      cards: Array.from({ length: count }, (_, i) => ({
        id: nextId++,
        title: `${title} ${i + 1}`,
        imageUrl: `https://picsum.photos/seed/${seed}-${i}/200/300`,
      })),
    })),
  };
}

const canvas = document.querySelector<HTMLCanvasElement>('#carousel')!;
const handle = mountCarousels(canvas, JSON.stringify(sampleData()));

// Demonstrate teardown: unmount if the canvas is ever removed from the page.
window.addEventListener('beforeunload', () => handle.unmount());
