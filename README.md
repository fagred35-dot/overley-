# NOVA Crosshair

Лёгкий open-source игровой оверлей-прицел. **Реализованы Фазы 1–2:** каркас desktop-приложения, независимое click-through окно, трей, каталог из 50 различных форм, live preview и настройки внешнего вида.

> **Важно:** внешний оверлей не взаимодействует с процессом игры, но это не является гарантией одобрения любым античитом. Правила Vanguard, EAC, BattlEye и конкретной игры меняются; перед использованием проверьте их актуальные условия. В эксклюзивном fullscreen ОС или композитор могут не показывать сторонние окна — переключите игру в Borderless / «Без рамки». Автоопределение режима экрана ещё не реализовано.

## Реализовано

- Панель настроек с тёмной glassmorphism-темой и неоновым акцентом; закрытие окна скрывает его в трей.
- Click-through оверлей поверх основного монитора:
  - **Windows:** прозрачное topmost-окно Tauri/Wry с нативной обработкой прозрачности и игнорированием курсора.
  - **macOS:** отдельное прозрачное floating-окно, игнорирующее мышь.
  - **Linux/X11:** GTK/Cairo, X11 `override_redirect` и pass-through области ввода.
  - **Linux/Wayland:** GTK/Cairo в слое `overlay` через `gtk-layer-shell`; при отсутствии протокола интерфейс показывает предупреждение.
- **50 геометрий**, а не перекраски одной формы: кресты, точки, кольца, T/L, шевроны, снайперские сетки, динамические, пиксельные, минималистичные, гибридные и анимированные пресеты. Каждый хранится в `src/presets.json` вместе со слоями, геометрией и параметрами.
- Поиск, фильтрация по 11 категориям, live preview и настройки цвета, размера, толщины, центрального зазора, непрозрачности, поворота, обводки и свечения.
- Настройки сохраняются в пользовательский config-файл приложения и восстанавливаются при следующем запуске.
- Трей: показать/скрыть оверлей, открыть настройки, выход.
- Установочные форматы сконфигурированы для Windows (NSIS `.exe`), macOS (`.dmg`) и Linux (`.AppImage`, `.deb`, `.rpm`). Каждый пакет следует собирать на целевой ОС.

### Ещё не реализовано

Наблюдатель процессов и правил «игра → профиль», профили, выбор монитора активной игры, пользовательские PNG/JPG/SVG, замена системного курсора, глобальные хоткеи, автозапуск и автообновление — следующие фазы. Анимационные метаданные и preview готовы; на Linux нативная Cairo-поверхность пока рисует эти пресеты статично.

## Стек

- **Tauri 2 + Rust** — нативные окна, трей, системное хранение настроек; UI — HTML/CSS/JS через Vite без Electron и встроенного полного Chromium.
- Windows/macOS отображают прицел в отдельной минимальной WebView. Linux использует GTK/Cairo, чтобы применять X11 override-redirect и Wayland layer-shell без графических хуков.
- Состояние между UI и оверлеем передаётся событиями; циклов polling нет. Статичные пресеты не перерисовываются по таймеру.
- В приложении нет DLL-инъекций, чтения/записи памяти игры, kernel-компонентов или перехвата DirectX/OpenGL/Vulkan.

Фактический расход RAM зависит от ОС и системного WebView/композитора. **Целевые лимиты 30/50 МБ не подтверждены профилированием и не гарантируются** для Tauri/WebView на всех платформах.

## Структура

```text
.
├── README.md
├── LICENSE
├── package.json                 # Vite и Tauri CLI
├── vite.config.js
├── src/
│   ├── index.html               # панель управления, каталог и настройки
│   ├── main.js
│   ├── styles.css
│   ├── phase2.css
│   ├── crosshair.css
│   ├── presets.json             # 50 JSON-пресетов + Cairo/SVG-геометрия
│   ├── overlay.html
│   ├── overlay.js
│   └── overlay.css
└── src-tauri/
    ├── Cargo.toml
    ├── build.rs
    ├── tauri.conf.json
    ├── capabilities/default.json
    ├── icons/
    └── src/
        ├── main.rs
        ├── lib.rs               # Tauri-команды, tray и состояние
        ├── config.rs            # модель/валидация 50 пресетов и настроек
        └── linux_overlay.rs     # GTK/Cairo, X11 и Wayland backend
```

## Сборка из исходников

### Общее

Установите Rust stable через `rustup`, Node.js 18+ и npm.

```sh
npm install
npm run tauri -- dev
```

Для production-сборки:

```sh
npm run tauri -- build
```

Артефакты создаются в `src-tauri/target/release/bundle/`. `npm run dev` запускает только веб-предпросмотр интерфейса и не создаёт нативное системное окно оверлея.

### Windows 10/11

Установите Visual Studio Build Tools с C++ workload и WebView2 Runtime. В PowerShell из корня проекта:

```powershell
npm install
npm run tauri -- build --bundles nsis
```

NSIS установщик `.exe` появится в `src-tauri/target/release/bundle/nsis/`.

### macOS 12+

Нужны Xcode Command Line Tools. Сборка текущей архитектуры:

```sh
npm install
npm run tauri -- build --bundles dmg
```

Для universal-сборки установите обе Rust-цели и запустите CLI на macOS:

```sh
rustup target add aarch64-apple-darwin x86_64-apple-darwin
npm run tauri -- build --target universal-apple-darwin --bundles dmg
```

Подписание и notarization публичного приложения требуют сертификатов Apple; секреты в репозиторий не добавляйте.

### Linux (X11 и Wayland)

Нужны GTK 3, WebKitGTK 4.1, AppIndicator, X11 dev headers и `gtk-layer-shell` (runtime и headers). Для Debian/Ubuntu:

```sh
sudo apt install build-essential pkg-config libssl-dev libgtk-3-dev \
  libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev \
  libx11-dev libgtk-layer-shell-dev patchelf
npm install
npm run tauri -- dev
```

Пакеты собираются на Linux:

```sh
npm run tauri -- build --bundles appimage
npm run tauri -- build --bundles deb
npm run tauri -- build --bundles rpm
```

`.deb` объявляет зависимость `libgtk-layer-shell0`, RPM — `gtk-layer-shell`. На Wayland нужен layer-shell-совместимый композитор. На X11 `override_redirect` не гарантирует отображение поверх эксклюзивного fullscreen во всех оконных менеджерах.


## Совместимость с античитами и ограничения

NOVA — самостоятельное окно ОС; нет внедрения в игровой процесс, чтения/записи памяти, kernel-компонентов, графических хуков или автоматизации игрового ввода. Это уменьшает технические риски, но не является сертификатом совместимости: античит или правила игры могут запрещать оверлеи. Не обходите блокировки и соблюдайте правила игры.

Прицел пока центрируется на **основном мониторе**. Автоопределения exclusive fullscreen нет; если прицел не виден, переключите игру в Borderless/оконный режим. На Wayland слой overlay доступен только при поддержке layer-shell со стороны композитора.

## Лицензия

MIT. См. [LICENSE](LICENSE).
