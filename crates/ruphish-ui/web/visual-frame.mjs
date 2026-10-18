import {Jodit} from 'jodit';
import 'jodit/es2021/jodit.min.css';
import {installVisualController} from './visual-controller.mjs';

installVisualController(window, (element, options) => Jodit.make(element, options));
