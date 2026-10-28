import { Jodit } from 'jodit';
import german from 'jodit/esm/langs/de.js';
import 'jodit/es2021/jodit.min.css';
import '../theme.css';
import { installVisualController } from './visual-controller.mjs';

installVisualController(window, (element, options) =>
  Jodit.make(element, { ...options, i18n: { de: german } }),
);
