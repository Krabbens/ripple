export const styles = `
:host { all: initial; --ink: #2f3437; --muted: #787774; --line: #eaeaea; --surface: #fff; --canvas: #f7f6f3; --blue: #1f6c9f; --blue-wash: #e1f3fe; font: 14px/1.6 'SF Pro Display', 'Helvetica Neue', Arial, sans-serif; color: var(--ink); }
* { box-sizing: border-box; }
button, textarea, a { font: inherit; }
button, a { -webkit-tap-highlight-color: transparent; }
button { cursor: pointer; display: inline-flex; align-items: center; justify-content: center; gap: 6px; min-height: 38px; border: 1px solid var(--line); color: var(--ink); background: var(--surface); border-radius: 6px; padding: 7px 10px; }
button:hover { background: var(--canvas); }
button:active { transform: scale(.98); }
button:focus-visible, a:focus-visible, textarea:focus-visible { outline: 2px solid var(--blue); outline-offset: 3px; }
button:disabled { opacity: .4; cursor: default; }
button[aria-pressed=true] { background: var(--blue-wash); border-color: var(--blue-wash); color: var(--blue); }
.icon { display: block; width: 18px; height: 18px; flex-shrink: 0; }
.panel { position: fixed; right: 20px; top: 20px; bottom: 20px; width: 320px; padding: 22px; background: var(--surface); border: 1px solid var(--line); border-radius: 12px; pointer-events: auto; display: flex; flex-direction: column; overflow-y: auto; }
.top, .row { display: flex; align-items: center; justify-content: space-between; gap: 8px; }
.panel[data-collapsed=true] { bottom: auto; padding: 14px 18px; }
.panel[data-collapsed=true] > :not(.top) { display: none; }
[data-collapse] { min-height: 30px; width: 30px; padding: 4px; border: 0; }
.brand { display: flex; align-items: center; gap: 9px; font-size: 21px; font-weight: 600; letter-spacing: -.8px; }
.brand .icon { width: 24px; height: 24px; }
.badge { margin-left: auto; font-size: 12px; color: var(--muted); }
.intro { color: var(--muted); margin: 20px 0 16px; }
.tools { display: grid; grid-template-columns: repeat(3, 1fr); gap: 4px; margin-bottom: 22px; }
.tools button { flex-direction: column; gap: 6px; border: 0; font-size: 12px; min-height: 64px; padding: 10px 3px; }
.label { display: flex; align-items: center; justify-content: space-between; font-size: 12px; color: var(--muted); margin-bottom: 8px; }
kbd { border: 1px solid var(--line); border-radius: 4px; background: var(--canvas); font: 11px/1.5 'SF Mono', monospace; padding: 1px 5px; }
.selection { padding: 10px 12px; min-height: 44px; border: 1px solid var(--line); border-radius: 6px; font: 12px/1.5 'SF Mono', monospace; display: flex; justify-content: space-between; align-items: center; gap: 8px; overflow-wrap: anywhere; }
.selection button { min-height: 26px; padding: 3px; border: 0; }
.hint { margin: 8px 0 0; color: var(--muted); font-size: 12px; }
.compose { display: flex; flex-direction: column; flex: 1; min-height: 180px; margin-top: 26px; }
textarea { flex: 1; width: 100%; min-height: 120px; resize: none; background: var(--surface); color: var(--ink); border: 1px solid var(--line); border-radius: 6px; padding: 12px; line-height: 1.6; }
textarea::placeholder { color: var(--muted); }
.meta { margin: 10px 0 16px; color: var(--muted); font-size: 11px; }
.meta button { min-height: 26px; background: none; border: 0; padding: 3px 5px; font-size: 11px; }
.send { background: #111; border-color: #111; color: #fff; font-weight: 500; width: 100%; min-height: 44px; }
.send:hover { background: #333; }
.send .icon { margin-left: auto; }
.model { display: flex; justify-content: space-between; margin-top: 22px; padding-top: 16px; border-top: 1px solid var(--line); font-size: 12px; }
.model span:last-child { color: var(--muted); }
.account { margin-top: 14px; }
.account a { display: block; padding: 9px 12px; text-align: center; border: 1px solid var(--line); border-radius: 6px; color: var(--ink); text-decoration: none; font-size: 12px; overflow-wrap: anywhere; }
.account a:hover { background: var(--canvas); }
.source { margin: 12px 0 0; color: var(--muted); font: 11px/1.5 'SF Mono', monospace; overflow-wrap: anywhere; }
.status { font-size: 12px; margin-top: 12px; padding: 10px; background: #edf3ec; color: #346538; border-radius: 6px; overflow-wrap: anywhere; white-space: pre-line; max-height: 180px; overflow-y: auto; flex-shrink: 0; }
.status:empty { display: none; }
.status[data-error=true] { background: #fdebec; color: #9f2f2d; }
.outline { position: fixed; pointer-events: none; border: 2px solid var(--blue); border-radius: 2px; display: none; }
.drop { position: fixed; height: 3px; pointer-events: none; background: var(--blue); display: none; }
.draw-layer { position: fixed; inset: 0; width: 100vw; height: 100vh; pointer-events: none; overflow: visible; }
@media (prefers-reduced-motion: no-preference) { button { transition: transform 120ms ease; } .panel { animation: panel-enter 220ms cubic-bezier(.16,1,.3,1); } @keyframes panel-enter { from { opacity: 0; transform: translateY(8px); } to { opacity: 1; transform: translateY(0); } } }
@media (prefers-reduced-motion: reduce) { *, *::before, *::after { animation: none !important; transition: none !important; } }
@media (max-width: 979px) { .panel { top: auto; right: 16px; bottom: 16px; width: min(360px, calc(100vw - 32px)); max-height: calc(100dvh - 32px); padding: 18px; } .panel[data-collapsed=true] { top: auto; bottom: 16px; } .intro { margin: 12px 0; } .tools { margin-bottom: 14px; } .tools button { flex-direction: row; min-height: 44px; } .compose { flex: none; min-height: 0; margin-top: 16px; } textarea { min-height: 88px; height: 88px; } .model { margin-top: 14px; padding-top: 12px; } }
`;
