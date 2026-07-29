import './style.css';
import { setupApp } from 'rs-wasm-tv-app';
import { createHtml5Player } from './player';

setupApp(document.querySelector('#app')!, createHtml5Player());
