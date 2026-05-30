import hljs from 'highlight.js/lib/core';
import bash from 'highlight.js/lib/languages/bash';
import css from 'highlight.js/lib/languages/css';
import dockerfile from 'highlight.js/lib/languages/dockerfile';
import javascript from 'highlight.js/lib/languages/javascript';
import json from 'highlight.js/lib/languages/json';
import markdown from 'highlight.js/lib/languages/markdown';
import plaintext from 'highlight.js/lib/languages/plaintext';
import rust from 'highlight.js/lib/languages/rust';
import sql from 'highlight.js/lib/languages/sql';
import typescript from 'highlight.js/lib/languages/typescript';
import xml from 'highlight.js/lib/languages/xml';
import yaml from 'highlight.js/lib/languages/yaml';

hljs.registerLanguage('bash', bash);
hljs.registerLanguage('sh', bash);
hljs.registerLanguage('shell', bash);
hljs.registerLanguage('css', css);
hljs.registerLanguage('dockerfile', dockerfile);
hljs.registerLanguage('javascript', javascript);
hljs.registerLanguage('js', javascript);
hljs.registerLanguage('jsx', javascript);
hljs.registerLanguage('json', json);
hljs.registerLanguage('markdown', markdown);
hljs.registerLanguage('md', markdown);
hljs.registerLanguage('plaintext', plaintext);
hljs.registerLanguage('text', plaintext);
hljs.registerLanguage('rust', rust);
hljs.registerLanguage('rs', rust);
hljs.registerLanguage('sql', sql);
hljs.registerLanguage('typescript', typescript);
hljs.registerLanguage('ts', typescript);
hljs.registerLanguage('tsx', typescript);
hljs.registerLanguage('html', xml);
hljs.registerLanguage('xml', xml);
hljs.registerLanguage('yaml', yaml);
hljs.registerLanguage('yml', yaml);

const extensionMap: Record<string, string> = {
  cjs: 'javascript',
  css: 'css',
  dockerfile: 'dockerfile',
  html: 'html',
  js: 'javascript',
  json: 'json',
  jsx: 'javascript',
  md: 'markdown',
  mjs: 'javascript',
  rs: 'rust',
  sh: 'bash',
  sql: 'sql',
  toml: 'plaintext',
  ts: 'typescript',
  tsx: 'typescript',
  txt: 'plaintext',
  xml: 'xml',
  yaml: 'yaml',
  yml: 'yaml',
};

function extensionFromPath(path: string) {
  const lastSegment = path.split('/').pop() ?? path;
  if (lastSegment.toLowerCase() === 'dockerfile') {
    return 'dockerfile';
  }

  const dotIndex = lastSegment.lastIndexOf('.');
  if (dotIndex === -1) {
    return '';
  }

  return lastSegment.slice(dotIndex + 1).toLowerCase();
}

export function normalizeLanguage(language?: string, path = '') {
  const candidate = (language || extensionMap[extensionFromPath(path)] || 'plaintext').toLowerCase();

  if (hljs.getLanguage(candidate)) {
    return candidate;
  }

  return extensionMap[extensionFromPath(path)] || 'plaintext';
}

export function highlightSource(source: string, language?: string, path = '') {
  const normalizedLanguage = normalizeLanguage(language, path);
  const highlighted = hljs.getLanguage(normalizedLanguage)
    ? hljs.highlight(source, { language: normalizedLanguage, ignoreIllegals: true }).value
    : hljs.highlightAuto(source).value;

  return {
    language: normalizedLanguage,
    html: highlighted,
  };
}

export function formatFileSize(value?: number | null) {
  if (!value) {
    return '0 B';
  }

  if (value < 1024) {
    return `${value} B`;
  }

  if (value < 1024 * 1024) {
    return `${(value / 1024).toFixed(1)} KB`;
  }

  return `${(value / (1024 * 1024)).toFixed(1)} MB`;
}
