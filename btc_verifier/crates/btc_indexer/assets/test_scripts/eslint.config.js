import { Linter } from 'eslint';
import typescriptEslintPlugin from '@typescript-eslint/eslint-plugin';
import typescriptEslintParser from '@typescript-eslint/parser';

/** @type {Linter.FlatConfig} */
const config = [
  {
    languageOptions: {
      parser: typescriptEslintParser,
      ecmaVersion: 2020,
      sourceType: 'module',
    },
    plugins: {
      '@typescript-eslint': typescriptEslintPlugin,
    },
    rules: {
      semi: ['error', 'always'],
      quotes: ['error', 'single'],
      indent: ['error', 2],
      'no-unused-vars': 'off',
      '@typescript-eslint/no-unused-vars': ['error'],
    }
  },
];

export default config;