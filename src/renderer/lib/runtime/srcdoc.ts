/**
 * Wrap an artifact HTML body for a sandboxed iframe.
 *
 * Adapted from open-design `src/runtime/srcdoc.ts` (Apache-2.0).
 * The Rust crate `design-artifact::srcdoc` has its own wrapper that
 * runs server-side; this module is the browser-side counterpart for
 * the cases where the renderer wants to skip a backend round-trip
 * (e.g. the PreviewIframe in Phase 5b-11 takes the in-flight
 * artifact body straight from the artifacts slice).
 *
 * Differences from the upstream version:
 * - Deck bridge (slide nav postMessage protocol) is omitted; Phase
 *   5c re-adds it.
 * - Console-capture script `posts` to the parent on the channel
 *   the rest of kangnam-sdk uses (`design-artifact-iframe`),
 *   matching the Rust wrapper's contract.
 */
export interface SrcdocOptions {
  /** `<base href>` so relative URLs resolve against the artifact dir. */
  baseHref?: string
  /** Frontend channel name posted in window.parent.postMessage. */
  errorChannel?: string
  /**
   * If true and the body already starts with `<!doctype` / `<html`,
   * passes through unchanged. Mirrors the Rust crate option of the
   * same name.
   */
  passthroughFullDocuments?: boolean
}

const DEFAULT_CHANNEL = 'design-artifact-iframe'

export function buildSrcdoc(html: string, opts: SrcdocOptions = {}): string {
  const trimmed = html.trimStart()
  const head = trimmed.slice(0, 64).toLowerCase()
  const isFullDoc = head.startsWith('<!doctype') || head.startsWith('<html')
  if (isFullDoc && opts.passthroughFullDocuments) return html

  const channel = opts.errorChannel ?? DEFAULT_CHANNEL
  const wrapped = isFullDoc
    ? html
    : `<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
  </head>
  <body>${html}</body>
</html>`
  const withBase = opts.baseHref ? injectBaseHref(wrapped, opts.baseHref) : wrapped
  const withShim = injectSandboxShim(withBase)
  return injectErrorBridge(withShim, channel)
}

function injectBaseHref(doc: string, baseHref: string): string {
  const safeHref = escapeAttr(baseHref)
  const tag = `<base href="${safeHref}">`
  if (/<head[^>]*>/i.test(doc)) return doc.replace(/<head[^>]*>/i, (m) => `${m}${tag}`)
  if (/<html[^>]*>/i.test(doc))
    return doc.replace(/<html[^>]*>/i, (m) => `${m}<head>${tag}</head>`)
  return tag + doc
}

function escapeAttr(value: string): string {
  return value
    .replace(/&/g, '&amp;')
    .replace(/"/g, '&quot;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
}

/**
 * `sandbox="allow-scripts"` without `allow-same-origin` raises a
 * SecurityError on the first localStorage/sessionStorage access.
 * Many AI-generated decks/landing pages call `localStorage.getItem(…)`
 * at the top of their IIFE without try/catch — when it throws, the
 * whole script aborts and the artifact becomes static. Install an
 * in-memory shim *before* any user script runs.
 */
function injectSandboxShim(doc: string): string {
  const shim = `<script>(function(){
  function makeStore(){
    var data = {};
    var api = {
      getItem: function(k){ return Object.prototype.hasOwnProperty.call(data, k) ? data[k] : null; },
      setItem: function(k, v){ data[k] = String(v); },
      removeItem: function(k){ delete data[k]; },
      clear: function(){ data = {}; },
      key: function(i){ return Object.keys(data)[i] || null; }
    };
    Object.defineProperty(api, 'length', { get: function(){ return Object.keys(data).length; } });
    return api;
  }
  function tryShim(name){
    var works = false;
    try { works = !!window[name] && typeof window[name].getItem === 'function'; void window[name].length; }
    catch (_) { works = false; }
    if (works) return;
    try { Object.defineProperty(window, name, { configurable: true, value: makeStore() }); }
    catch (_) { try { window[name] = makeStore(); } catch (__) {} }
  }
  tryShim('localStorage');
  tryShim('sessionStorage');
})();</script>`
  if (/<head[^>]*>/i.test(doc)) return doc.replace(/<head[^>]*>/i, (m) => `${m}${shim}`)
  if (/<body[^>]*>/i.test(doc)) return doc.replace(/<body[^>]*>/i, (m) => `${m}${shim}`)
  return shim + doc
}

/**
 * Forward `console.*` calls and runtime errors to the parent window
 * via postMessage on `channel`. The PreviewIframe component (5b-11)
 * listens for this and surfaces the messages so the design `preview`
 * tool can include them in the response payload sent back to the
 * agent (`{ console, errors }`).
 */
function injectErrorBridge(doc: string, channel: string): string {
  const literal = JSON.stringify(channel)
  const bridge = `<script>(function () {
  var channel = ${literal};
  function post(payload) {
    try {
      window.parent.postMessage(Object.assign({ channel: channel }, payload), '*');
    } catch (e) { /* sandboxed: silently drop */ }
  }
  window.addEventListener('error', function (ev) {
    post({ kind: 'error', message: String(ev.message || ev), source: ev.filename, line: ev.lineno, col: ev.colno });
  });
  window.addEventListener('unhandledrejection', function (ev) {
    post({ kind: 'unhandled-rejection', reason: String(ev.reason || '') });
  });
  ['log','info','warn','error','debug'].forEach(function (level) {
    var orig = console[level];
    console[level] = function () {
      try {
        var parts = Array.prototype.slice.call(arguments).map(function (a) {
          try { return typeof a === 'string' ? a : JSON.stringify(a); } catch (e) { return String(a); }
        });
        post({ kind: 'console', level: level, message: parts.join(' ') });
      } catch (e) {}
      try { orig.apply(console, arguments); } catch (e) {}
    };
  });
  window.addEventListener('DOMContentLoaded', function () {
    post({ kind: 'ready' });
  });
})();</script>`
  if (/<head[^>]*>/i.test(doc)) return doc.replace(/<head[^>]*>/i, (m) => `${m}${bridge}`)
  if (/<body[^>]*>/i.test(doc)) return doc.replace(/<body[^>]*>/i, (m) => `${m}${bridge}`)
  return bridge + doc
}
