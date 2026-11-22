# Third-party components

VELIORA App ships the following open-source programs. They run as separate
processes started by VELIORA App; their files are distributed unmodified
(TgWsProxy is rebuilt from unmodified source, see `docs/COMPONENTS.md`).
Exact versions and SHA-256 hashes are recorded in
`src-tauri/components/components.json`. Full license texts are included in
`src-tauri/components/licenses/` and are installed together with the app.

| Component | Use in VELIORA App | License | Source |
|---|---|---|---|
| Flowseal / tg-ws-proxy 1.11.4 | Local Telegram MTProto → WebSocket proxy (`components/tgws/TgWsProxy.exe`) | MIT | https://github.com/Flowseal/tg-ws-proxy |
| Flowseal / zapret-discord-youtube 1.10.4 | Strategy scripts, host/IP lists, fake packet samples (`components/zapret/`) | MIT | https://github.com/Flowseal/zapret-discord-youtube |
| bol-van / zapret (`winws.exe`) | DPI bypass engine | MIT | https://github.com/bol-van/zapret |
| WinDivert (`WinDivert.dll`, `WinDivert64.sys`) | Packet capture driver, unmodified binaries | LGPL-3.0 or GPL-2.0 (your choice) | https://github.com/basil00/WinDivert |
| Cygwin (`cygwin1.dll`) | Runtime of `winws.exe`, unmodified binary | LGPL-3.0 | https://cygwin.com (source: https://cygwin.com/git/) |
| PyInstaller bootloader, CPython, cryptography, httpx, h2, certifi | Runtime bundled inside `TgWsProxy.exe` | GPL-2.0 with bootloader exception, PSF-2.0, Apache-2.0 / BSD-3-Clause, BSD-3-Clause, BSD-3-Clause, MPL-2.0 | see each project |

LGPL/GPL components (WinDivert, Cygwin) are redistributed as unmodified
binaries; their corresponding source code is available at the links above.
VELIORA App's own code does not include code from these projects — it starts
the programs and passes them command-line arguments.

Behaviour of the Telegram/Zapret features was modelled on the open-source
Slipgate project (https://github.com/tenstepsbeforedecay/Slipgate, GPL-3.0);
no Slipgate source code, text or artwork is included.

The cursor trail (`src/components/SplashCursor/`) is based on the SplashCursor
component from React Bits (https://reactbits.dev, MIT + Commons Clause).

The Telegram Proxy catalog loads TDLib 1.8.70 (Boost Software License 1.0),
built from tdlib/td commit c8828d50f5eac2fc7089ce1089d7ce9095c61b5e.
The Windows x64 binary is distributed by the tdl project in
@prebuilt-tdlib/win32-x64 0.1008070.0 (package license 0BSD).
The installer includes both license texts under tdlib/.
Sources: https://github.com/tdlib/td and https://github.com/eilvelia/tdl .
