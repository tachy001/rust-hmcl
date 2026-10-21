import { createApp } from 'vue';
import App from './App.vue';
import './style.css';
import './aster.css';
// Suppress the WebView's browser menu without stopping our instance menu handlers.
document.addEventListener('contextmenu', (event) => event.preventDefault(), { capture: true });
createApp(App).mount('#app');
