<div align="center">

<img src="assets/brand/logo-dark.svg" alt="VELIORA" width="360">

**Modern Windows application for proxy management and network tools.**

Telegram и свободный интернет в одном окне — для Windows 10/11 x64.

[**Скачать последнюю версию**](../../releases/latest) · [Telegram-канал insomnia project](https://t.me/insomniatype) · [Сообщить о проблеме](../../issues) · [История версий](CHANGELOG.md)

</div>

![VELIORA App — главный экран](docs/img/v21-home-light.png)

## В разработке — VELIORA 2.2.5

Готовим обновление для Windows x64. В нём появятся автоматический подбор прокси для Telegram, каталог с резервными источниками, дополнительные режимы для Discord и более надёжное восстановление системного прокси.

Сборка 2.2.5 проходит проверку. Для публичной загрузки пока доступна [версия 2.1.0](https://github.com/gercikdaniil6-stack/VELIORA/releases/tag/v2.1.0). О выходе обновления сообщим в [insomnia project](https://t.me/insomniatype).

## Возможности

- **Telegram-прокси.** Локальный MTProto-прокси (Flowseal/tg-ws-proxy): Telegram подключается к нему по ссылке `tg://proxy`, трафик идёт через WebSocket на веб-узлы Telegram, с запасным каналом через Cloudflare и поддержкой своего Cloudflare Worker. Ссылка добавляется в Telegram в один клик и не меняется между перезапусками.
- **Обход DPI на базе Zapret.** winws + WinDivert со стратегиями Flowseal/zapret-discord-youtube: YouTube, Discord и игры без VPN. Встроенный тест проверяет все 22 стратегии на вашей сети, перепроверяет лидеров и отмечает лучшую. Есть игровой фильтр, сменные фейки, режимы IPSet, очистка кеша Discord, установка версий сборки с проверкой SHA-256.
- **Списки Zapret.** Редактор доменов и IP (свои списки и списки сборки), импорт и экспорт, бэкап, заводской сброс. Свои записи переносятся при обновлении сборки.
- **Системный прокси.** Профили HTTP/SOCKS, правила обхода, восстановление прежних настроек Windows.
- **Диагностика «Почему может не работать?».** Находит частые причины (другая программа обхода, отключённая служба Windows, VPN, занятый порт) и даёт пошаговую инструкцию.
- **Журнал** с фильтрами и экспортом, **трей** с быстрыми переключателями, **автозапуск** с Windows без запроса UAC.
- **Оформление.** 14 тем (7 светлых, 5 тёмных, System, Custom), Liquid Glass с бликами и преломлением, анимации и жидкий след за курсором за стеклом. Подробно — [docs/APPEARANCE.md](docs/APPEARANCE.md).

| Telegram | Zapret | Оформление |
|---|---|---|
| ![Telegram](docs/img/v21-telegram.png) | ![Zapret](docs/img/v21-zapret.png) | ![Оформление](docs/img/v21-appearance.png) |

## Установка

1. Скачайте `VELIORA App_<версия>_x64-setup.exe` со страницы [Releases](../../releases/latest).
2. Сверьте SHA-256 файла с указанным в описании релиза: `Get-FileHash .\VELIORA*.exe -Algorithm SHA256`.
3. Запустите установщик. Приложение ставится в Program Files, для этого нужны права администратора.

При запуске VELIORA App просит права администратора. Без них работают Telegram-прокси и системный прокси, а Zapret нет: драйверу WinDivert нужны права. Если включить «Запускать вместе с Windows», приложение стартует через Планировщик заданий сразу с правами, без окна UAC.

Установщик не подписан сертификатом издателя, поэтому Windows SmartScreen может показать предупреждение «Неизвестный издатель». Обход DPI работает через драйвер перехвата пакетов, и некоторые антивирусы реагируют на такие программы (winws, WinDivert). Результат проверки VirusTotal для каждой сборки указан в описании релиза.

## Сборка из исходников

Нужны Node.js 20+, Rust (stable, MSVC) и Microsoft C++ Build Tools, а также WebView2 (есть в Windows 10/11).

```bash
npm install            # зависимости фронтенда
npm run app:dev        # режим разработки (для проверки Zapret запустите терминал от администратора)
npm run app:build      # exe + установщик NSIS в src-tauri/target/release/bundle/nsis/
```

Публичный релиз собирается скриптом `scripts/release.ps1`: он убирает из exe локальные пути машины сборки, проверяет результат и печатает SHA-256 установщика.

```powershell
powershell -ExecutionPolicy Bypass -File scripts\release.ps1
```

Прочие команды:

```bash
npm run components     # заново скачать сборку Zapret (официальный релиз, сверка SHA-256)
npm run brand          # пересобрать логотипы и иконки
```

Telegram-прокси пересобирается из исходников командой `powershell -ExecutionPolicy Bypass -File scripts\build-tgws.ps1` (подробности — [docs/COMPONENTS.md](docs/COMPONENTS.md)).

Тесты: `cd src-tauri && cargo test`. Команда `cargo test -- --ignored winws` дополнительно проверяет аргументы всех стратегий на настоящем `winws.exe` в режиме `--dry-run`.

## Структура

```
src-tauri/src/
  lib.rs        команды, трей, окно, автозапуск компонентов, права администратора
  component.rs  надзор за дочерним процессом: job object, журнал, статус
  tgws.rs       Telegram-прокси: запуск, порт, ссылка
  zapret.rs     Zapret: подготовка файлов, разбор стратегий, игровой фильтр, IPSet, списки, кеш Discord
  tester.rs     тест стратегий
  updates.rs    версии сборки Zapret с GitHub (SHA-256)
  diag.rs       диагностика
  autorun.rs    запуск с Windows (Планировщик заданий)
  config.rs     настройки (config.json)
  logs.rs       журнал
  proxy.rs      системный прокси Windows
  win.rs        Win32: процессы, службы, права, адаптеры
src-tauri/components/   TgWsProxy.exe, сборка Zapret, лицензии, components.json (версии и хэши)
src/pages/              Главная, Telegram, Zapret, Списки, Системный прокси, Журнал, Оформление, Настройки, О приложении
src/theme/              темы и движок оформления
docs/                   компоненты, оформление, бренд, дизайн-система, пример настроек
```

## Безопасность и данные

- Приложение не собирает телеметрию и не отправляет ваши данные. Сетевые запросы: проверка версий компонентов через GitHub API (можно отключить в настройках) и тест стратегий, который открывает сайты YouTube, Discord, Google, Cloudflare и Telegram Web.
- Настройки и журнал хранятся локально в `%LOCALAPPDATA%\app.veliora.desktop\`. `config.json` содержит ключ вашего Telegram-прокси, который генерируется случайно при первом запуске. Не публикуйте этот файл. Пример структуры — [docs/config.example.json](docs/config.example.json).
- Рабочая копия Zapret лежит в `%ProgramData%\VELIORA\zapret\`, писать туда могут только администраторы. Так ничто без прав не подменит программы, которые запускаются с правами.
- Сторонние компоненты поставляются без изменений, их хэши записаны в `src-tauri/components/components.json`.

Подробнее и как сообщить об уязвимости — [SECURITY.md](SECURITY.md).

## Новости и поддержка

- 📢 Новости, обновления и анонсы — Telegram-канал [insomnia project](https://t.me/insomniatype).
- 🐛 Нашли баг или что-то не работает — [создайте Issue](../../issues/new): опишите, что произошло, и приложите журнал (страница «Журнал» → «Сохранить в файл»).

## Лицензия

© 2026 VELIORA. Все права защищены — см. [LICENSE](LICENSE). Исходный код открыт для ознакомления и проверки. Официальные установщики из [Releases](../../releases/latest) можно бесплатно скачивать и использовать.

Сторонние компоненты распространяются по своим лицензиям: список и источники — [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md), тексты лицензий — `src-tauri/components/licenses/` (устанавливаются вместе с приложением).
