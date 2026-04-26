# Чего не хватает до «полноценного» веб-фреймворка (ресерч)

Документ фиксирует **разрыв** между текущим OxyRoute (RSGI + Rust hot path, см. [index](index.md)) и ожиданиями от **широкого** HTTP-фреймворка уровня FastAPI / Starlette / Django REST. Термин «полноценный» здесь означает **покрытие типичного продакшн-API и DX**, а не обязательность всех пунктов для твоего позиционирования.

---

## Уже есть (кратко)

- Маршрутизация по методам и путям (`matchit`), path/query/json/body, **405 / Allow**, **HEAD** на GET, **OPTIONS**.
- **JWT** (HS/RS/… через `jsonwebtoken`), iss/aud/leeway, cookie, **зависимости** с `request`, линейный порядок, `freeze`.
- **Response** / dict с заголовками и cookies, частичный **OpenAPI**, Pydantic/schema для тела.
- Один **pre-route middleware** (`set_middleware`), **ASGI мост** только для `http` ([asgi](asgi.md)).
- CI, PyPI, E2E Granian RSGI.

Ниже — то, чего **нет** или что **слабо** относительно «больших» фреймворков.

---

## 1. Протокол и транспорт

| Тема | Зазор | Комментарий |
|------|--------|-------------|
| **WebSockets** | Нет | RSGI/текущий стек заточен под HTTP-запрос/ответ. Нужна отдельная ветка согласно Granian/RSGI для WS и мост в Python. |
| **SSE / длинный стрим ответа** | Нет / ограничено | Сейчас ответы через `response_*` Granian; потоковая отдача чанками — отдельный дизайн. |
| **HTTP/2 push, trailers** | Не в фокусе | Обычно на стороне сервера; фреймворк редко экспонирует. |
| **ASGI: lifespan, `websocket`, background** | Частично | Мост ASGI — **только `http`**; нет полноценного ASGI-приложения с lifecycle из спеки. |

---

## 2. Запрос и тело

| Тема | Зазор | Комментарий |
|------|--------|-------------|
| **`multipart/form-data`** | Нет | Загрузка файлов, поля формы — нужен парсер и API (`upload`, `form()`). |
| **`application/x-www-form-urlencoded` (body)** | Нет | Сейчас разбор query есть; тело формы не инжектится как отдельный тип. |
| **Streaming request body** | Ограничено | Тело читается в буфер для JSON/сырых байт; большие upload без полного чтения — отдельная работа. |
| **Валидация на уровне фреймворка** | Частично | Pydantic удобнее вручную; нет единого `Body()` как в FastAPI для всех типов контента. |

---

## 3. Маршрутизация и композиция

| Тема | Зазор | Комментарий |
|------|--------|-------------|
| **Sub-routers / `include_router`** | Нет | Один плоский `App`; нет префиксов и вложенных без ручного дублирования путей. |
| **`Mount` / static files** | Нет | Отдача `/static` из каталога — обычно отдельный слой (nginx) или Starlette StaticFiles. |
| **Host-based routing** | Нет | |
| **Глобальные exception handlers** | Нет | Только 500 с JSON; нет `@app.exception_handler` и иерархии исключений. |
| **Middleware-цепочка** | Один хук | Нет порядка нескольких middleware и `on_request` / `on_response` как в ASGI-стеке. |

---

## 4. Безопасность (кроме JWT)

| Тема | Зазор | Комментарий |
|------|--------|-------------|
| **CORS** | Ручной | В доках упомянут preflight через `set_middleware`; нет готового `CORSMiddleware` с настройками. |
| **CSRF** | Нет | Для cookie-сессий и форм часто нужны токены. |
| **Rate limiting** | Нет | |
| **Security headers** (HSTS, CSP, …) | Ручные заголовки | Нет пресетов. |
| **JWKS / ротация ключей** | Частично | В бэклоге [#8](https://github.com/QueryaHub/OxyRoute/issues/8). |

---

## 5. Состояние приложения и сессии

| Тема | Зазор | Комментарий |
|------|--------|-------------|
| **Встроенный `app.state` / lifespan** | Слабо | `__rsgi_init__` и общий state — [#18](https://github.com/QueryaHub/OxyRoute/issues/18). |
| **Сессии (signed cookie, server-side)** | Нет | |
| **Кэш глобальных настроек** | Нет | Нет первого класса для config. |

---

## 6. Разработка и тестирование

| Тема | Зазор | Комментарий |
|------|--------|-------------|
| **TestClient** (как Starlette/FastAPI) | Нет | Тесты через httpx + ASGI/реальный Granian; нет единого обёрточного клиента из коробки. |
| **CLI** (`oxyroute dev`, scaffold) | Нет | |
| **OpenAPI генерация клиентов** | Частично | Документ есть; не обещается полная совместимость со всеми генераторами. |
| **Background tasks** | Нет | Нет `BackgroundTasks` после ответа; только внешний воркер/очередь. |

---

## 7. Наблюдаемость

| Тема | Зазор | Комментарий |
|------|--------|-------------|
| **Structured access log** | Зависит от Granian | Нет формата «как у фреймворка». |
| **Metrics (Prometheus)** | Нет | |
| **Tracing (OpenTelemetry)** | Нет | |

---

## 8. Экосистема и позиционирование

- **ORM / миграции** — не часть HTTP-фреймворка; обычно SQLAlchemy + Alembic отдельно.
- **Шаблоны HTML (Jinja)** — нет; для API не критично.
- **GraphQL / gRPC** — отдельные стеки.

OxyRoute осознанно **уже** в нише: **быстрый маршрут + JSON + JWT в Rust**. «Полноценность» для многих команд = **multipart, WebSocket, цепочка middleware, суброутеры, TestClient** — это хороший кандидат на дорожную карту, если цель — конкурировать с FastAPI по удобству, а не только по RSGI.

---

## Приоритизация (рекомендация)

1. **Состояние и lifecycle** [#18](https://github.com/QueryaHub/OxyRoute/issues/18) — без этого сложно делить конфиг по воркерам.  
2. **ASGI надёжность** [#17](https://github.com/QueryaHub/OxyRoute/issues/17) — если ASGI остаётся равноправным входом.  
3. **Перф роутера** [#4](https://github.com/QueryaHub/OxyRoute/issues/4) — при росте нагрузки.  
4. **Sub-routers или префиксы** — резко повышают пригодность для крупных приложений.  
5. **Multipart + form body** — если не только JSON API.  
6. **CORS / exception handlers** — быстрые победы на Python-стороне без ломки RSGI.

## Связанные GitHub-issues (критичные модули)

- [#46](https://github.com/QueryaHub/OxyRoute/issues/46) — sub-routers / `include_router` ([`bodies/21.md`](../.github/ISSUE_BACKLOG/bodies/21.md))  
- [#47](https://github.com/QueryaHub/OxyRoute/issues/47) — multipart и `application/x-www-form-urlencoded` ([`22.md`](../.github/ISSUE_BACKLOG/bodies/22.md))  
- [#48](https://github.com/QueryaHub/OxyRoute/issues/48) — `HTTPException` и глобальные обработчики ([`23.md`](../.github/ISSUE_BACKLOG/bodies/23.md))  
- [#49](https://github.com/QueryaHub/OxyRoute/issues/49) — CORS helper ([`24.md`](../.github/ISSUE_BACKLOG/bodies/24.md))  

---

[← Documentation index](index.md)
