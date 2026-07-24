/** JS-owned playback surface. Rust creates `#player-video`; this adapter drives it. */
export interface PlayerAdapter {
  loadAndPlay(url: string): void;
  play(): void;
  pause(): void;
  isPaused(): boolean;
  currentTime(): number;
  duration(): number;
  seek(t: number): void;
  setVisible(visible: boolean): void;
}

const VIDEO_ID = '#player-video';

function videoEl(): HTMLVideoElement {
  const el = document.querySelector(VIDEO_ID);
  if (!(el instanceof HTMLVideoElement)) {
    throw new Error(`${VIDEO_ID} missing — call after setupApp creates the stage`);
  }
  return el;
}

/** Dev stub: plain HTML5 `<video>`, same behaviour as the old Rust HtmlVideoSink. */
export function createHtml5Player(): PlayerAdapter {
  return {
    loadAndPlay(url: string) {
      const el = videoEl();
      const resolved = new URL(url, document.baseURI).href;
      const needsLoad = el.currentSrc !== resolved;
      if (needsLoad) {
        el.src = url;
        el.load();
      }
      // Always restart from 0 (demo cards share one static URL).
      const start = () => {
        el.currentTime = 0;
        void el.play();
      };
      if (!needsLoad || el.readyState >= HTMLMediaElement.HAVE_METADATA) {
        start();
      } else {
        el.addEventListener('loadedmetadata', start, { once: true });
      }
    },
    play() {
      void videoEl().play();
    },
    pause() {
      videoEl().pause();
    },
    isPaused() {
      return videoEl().paused;
    },
    currentTime() {
      return videoEl().currentTime;
    },
    duration() {
      const d = videoEl().duration;
      return Number.isFinite(d) ? d : 0;
    },
    seek(t: number) {
      videoEl().currentTime = t;
    },
    setVisible(visible: boolean) {
      videoEl().style.display = visible ? 'block' : 'none';
    },
  };
}
