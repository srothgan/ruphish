import js from '@eslint/js';
import globals from 'globals';

export default [
  {
    ignores: [
      '**/node_modules/**',
      '**/generated/**',
      '**/target/**',
      '**/dist/**',
      '**/coverage/**',
      '**/.git/**',
      '**/.astro/**',
      '**/.vercel/**',
      '.ci-tools/**',
      'data/**',
      'tests/acceptance/data/**',
      'tests/fixtures/**',
      'website/src/content/docs/**',
      'crates/ruphish-http/starter-templates/**',
    ],
  },
  {
    files: ['**/*.{js,mjs,cjs}'],
    languageOptions: { ecmaVersion: 'latest' },
    linterOptions: { reportUnusedDisableDirectives: 'error' },
    rules: {
      ...js.configs.recommended.rules,
      curly: ['error', 'all'],
      eqeqeq: ['error', 'always', { null: 'ignore' }],
      'no-var': 'error',
      'one-var': ['error', 'never'],
      'prefer-const': 'error',
      'no-unused-vars': ['error', { argsIgnorePattern: '^_', caughtErrorsIgnorePattern: '^_' }],
    },
  },
  {
    files: [
      '*.config.{js,mjs,cjs}',
      '**/*.cjs',
      '.github/scripts/**/*.{js,mjs}',
      'assets/website-demo/**/*.{js,mjs}',
      'demo/**/*.{js,mjs}',
      'tests/acceptance/**/*.{js,mjs}',
      'website/*.config.{js,mjs}',
      'website/tools/**/*.{js,mjs}',
      'crates/ruphish-ui/web/build*.mjs',
      'crates/ruphish-http/src/starter_templates/catalog.mjs',
      'crates/ruphish-ui/web/tests/**/*.{js,mjs}',
    ],
    languageOptions: { globals: globals.node },
  },
  {
    files: ['crates/ruphish-ui/web/*.mjs', 'website/src/**/*.{js,mjs}'],
    ignores: ['crates/ruphish-ui/web/build*.mjs'],
    languageOptions: { globals: globals.browser },
  },
  {
    // These Node harnesses also execute callbacks in a browser/JSDOM context.
    files: [
      'crates/ruphish-ui/web/tests/**/*.{js,mjs}',
      'assets/website-demo/capture.mjs',
      'assets/website-demo/record.mjs',
    ],
    languageOptions: { globals: globals.browser },
  },
  {
    files: ['crates/ruphish-adapters/src/landing-collector.js'],
    languageOptions: {
      sourceType: 'script',
      // The Rust landing-page renderer supplies these two bindings.
      globals: { ...globals.browser, endpoint: 'readonly', messages: 'readonly' },
    },
  },
  {
    files: ['crates/ruphish-http/src/starter_templates/interaction.js'],
    languageOptions: { sourceType: 'script', globals: globals.browser },
  },
];
