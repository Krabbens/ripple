export const styles = `
:host { all: initial; font: 13px/1.5 system-ui, sans-serif; color: #e8ece8; }
* { box-sizing: border-box; }
button, textarea, a { font: inherit; }
button, a { -webkit-tap-highlight-color: transparent; }
button { cursor: pointer; border: 1px solid #344640; color: inherit; background: #21332d; border-radius: 8px; padding: 8px 10px; }
button:hover { background: #30483d; }
button:focus-visible, a:focus-visible, textarea:focus-visible { outline: 2px solid #d2f87b; outline-offset: 3px; }
button:disabled { opacity: .4; cursor: default; }
button[aria-pressed=true] { background: #d2f87b; border-color: #d2f87b; color: #172c23; }
.panel { position: fixed; right: 24px; bottom: 24px; width: min(360px, calc(100vw - 32px)); padding: 18px; background: #14271f; border: 1px solid #405348; border-radius: 16px; box-shadow: 0 18px 65px #11241b38; pointer-events: auto; }
.top, .row { display: flex; align-items: center; justify-content: space-between; gap: 8px; }
.panel[data-collapsed=true] > :not(.top) { display: none; }
[data-collapse] { padding: 0 7px; }
.brand { font-size: 21px; font-weight: 650; letter-spacing: -.8px; }
.brand span { color: #d2f87b; }
.badge { font: 10px/1.4 ui-monospace, monospace; text-transform: uppercase; letter-spacing: 1.3px; color: #b7c5bb; }
.tools { display: flex; gap: 6px; margin: 16px 0 12px; }
.tools button { flex: 1; }
.selection { padding: 9px 11px; margin-bottom: 10px; min-height: 38px; border: 1px dashed #496052; border-radius: 8px; font: 12px/1.5 ui-monospace, monospace; color: #d2f87b; display: flex; justify-content: space-between; align-items: center; }
.selection button { padding: 0 7px; }
textarea { width: 100%; min-height: 82px; resize: vertical; background: #0d1e17; color: #f2f4ee; border: 1px solid #405348; border-radius: 9px; padding: 10px 12px; }
textarea::placeholder { color: #9cafa1; }
.meta { margin: 9px 0 13px; color: #9cafa1; font-size: 11px; }
.meta button { background: none; padding: 3px 6px; font-size: 11px; }
.send { background: #d2f87b; border-color: #d2f87b; color: #172c23; font-weight: 650; flex: 1; }
.send:hover { background: #e3ffa0; }
.account { margin-top: 13px; padding-top: 12px; border-top: 1px solid #34483b; color: #b7c5bb; font-size: 11px; }
.account a { color: #d2f87b; text-decoration: none; }
.status { font-size: 12px; color: #b7c5bb; margin-top: 10px; overflow-wrap: anywhere; }
.status:empty { display: none; }
.status[data-error=true] { color: #ffbcaa; }
.outline { position: fixed; pointer-events: none; border: 2px solid #8060d8; border-radius: 3px; background: #8060d808; display: none; }
.drop { position: fixed; height: 3px; pointer-events: none; background: #365b25; box-shadow: 0 0 0 2px #d2f87b; display: none; }
svg { position: fixed; inset: 0; width: 100vw; height: 100vh; pointer-events: none; overflow: visible; }
@media (max-width: 600px) { .panel { bottom: 12px; right: 16px; padding: 13px; } textarea { min-height: 56px; } }
`;
