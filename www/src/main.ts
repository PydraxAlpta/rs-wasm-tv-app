import './style.css';
import { setupApp } from 'rs-wasm-leanback';
import { createHtml5Player } from './player';

setupApp(document.querySelector('#app')!, createHtml5Player());
