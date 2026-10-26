# GUI 라이브러리 선정 및 조사

조사일: 2026-10-26. 이 문서는 `gui-plan.md`의 분석 계층, 대량 로그,
독립 보조 창, 설정과 플러그인 관리 요구사항에 맞춘 **확정 선택과 검증 후보**를 기록한다.
의존성은 아직 추가하지 않았다. 아래 호환성은 공식 manifest와 문서 확인을
뜻하며, aftercast에서의 컴파일 또는 Windows WebView2 실행 검증을 뜻하지 않는다.

## 확정한 선정 조건

2026-10-26 사용자 결정. 나머지 후보는 아래 제안을 바탕으로 선정한다.

| 조건 | 구현 기준 |
| --- | --- |
| 앱에서 직접 JS 연결을 작성하지 않는다 | ECharts·Tabulator 및 같은 목적으로 직접 작성하는 `document::eval`/Rust↔JS 메시지 연결을 제외한다. Dioxus 및 채택한 라이브러리가 내부적으로 사용하는 JS는 이 제한과 구분한다 |
| 다국어는 초기 요구사항이다 | GUI 초기 구성부터 `dioxus-i18n`을 도입한다. 처음부터 UI 문구를 번역 리소스로 관리하며 다국어 지원을 후속 작업으로 미루지 않는다 |
| SDK는 초기 기본 구성이다 | GUI 초기 구성부터 `dioxus-sdk`를 도입하고 필요한 기능을 활성화한다. 기능마다 자체 유틸리티를 만들기 전에 SDK를 확인한다. 전체 기능 활성화를 뜻하지는 않는다 |
| Tailwind 기반으로 구성한다 | Tailwind CSS v4와 Rust/UI를 공통 스타일·컴포넌트의 기본 조합으로 선택한다. 클래스 병합은 `tw_merge`, 아이콘은 Rust/UI와 같은 Lucide `icons`를 우선 사용한다 |
| 처음부터 비동기로 구현한다 | Host 호출과 파일 작업 등 대기 작업은 초기 GUI부터 비동기 흐름으로 구성한다. UI 스레드에서 `block_on`·동기 대기·긴 계산을 하지 않으며 로딩·오류·취소·오래된 응답 폐기·종료 대기를 함께 구현한다 |
| 애플리케이션 오류 보고는 `miette`를 사용한다 | `anyhow`를 도입하지 않는다. 기존 라이브러리의 구체적인 `thiserror` 오류 타입은 유지하고 애플리케이션 경계에서 진단과 문맥을 제공한다 |
| CLI 라이브러리는 `usage-rs`를 사용한다 | CLI 인자·서브커맨드·도움말과 `--aftercast-child` 내부 모드를 공통 `usage-rs` 파싱에 포함한다. 파싱 결과로 실행 모드를 분기하고, child 모드는 UI·앱 비동기 런타임 초기화 전에 실행한다 |
| Rust 정규식 라이브러리를 별도 도입하지 않는다 | 로그 정규식 검색과 ACT 트리거의 패턴 검증·실행은 managed 계층의 .NET `Regex`로 처리한다. GUI는 비동기 Host API로 요청하고 결과·진단을 표시한다 |
| 운영용 라이브러리를 모두 채택한다 | `tracing-subscriber`·`tracing-appender`·`directories`·`open`·`jiff`를 선정한다. GUI 설정 저장은 기존 `serde`와 `toml_edit`를 사용한다. GUI/CLI 실행 크레이트 구현 시 해당 의존성을 선언한다 |
| ACT 관련 파일은 실행 파일 옆에 둔다 | ACT 호환 설정·플러그인·관련 데이터와 앱이 생성하는 ACT 관련 로그/기록은 실행 파일 디렉터리와 그 하위의 기존 배치를 기준으로 둔다. 현재 작업 디렉터리나 OS 표준 앱 데이터 경로를 기준으로 삼지 않는다 |
| aftercast 설정 형식은 TOML이다 | 실행 파일 옆의 `config.toml`에 앱 소유 설정을 저장한다. `serde` + `toml_edit`로 읽고 변경 항목만 편집하여 주석·서식·알 수 없는 항목을 보존한다. ACT·플러그인의 기존 설정 형식은 유지한다 |

## 선정 조합과 검증 후보

| 영역 | 선택 또는 검증 후보 | 적용 이유와 경계 |
| --- | --- | --- |
| 실행기 | Dioxus Desktop의 WebView 렌더러 | 기존 GUI 계획을 유지하며 앱에서 직접 JS 연결을 작성하지 않는다 |
| 공통 위젯 | Rust/UI의 Dioxus 컴포넌트 | Tailwind 기반 소스를 프로젝트로 가져와 사용한다. 원본·라이선스·변경 내용을 추적하고, 각 컴포넌트의 Desktop 지원과 직접 JS 연결 필요 여부를 확인한다 |
| 스타일 | Tailwind CSS v4 + `tw_merge` | 앱 레이아웃과 위젯 스타일은 Tailwind로 통일하고 공통 테마 값은 CSS 변수로 관리한다. 외부 표·차트의 필수 CSS는 유지하며 테마 값을 맞춘다 |
| 아이콘 | Rust/UI의 Lucide `icons` | Rust/UI 컴포넌트와 같은 아이콘 API를 사용한다. 여러 Lucide 패키지를 동시에 도입하지 않는다 |
| 분석용 테이블 | `chorale-core`/`chorale-dioxus` 0.2.3 우선 실증 | 열 너비/순서/표시, 정렬, 선택, 가상화 등의 재구현을 피한다. Chorale는 실증할 첫 후보이며 운영 채택 확정은 아니다. 부족한 기능은 Rust/Dioxus API를 제공하는 대안을 검토하며 Tabulator로 전환하지 않는다 |
| 단순 로그 목록 가상화 | 공식 `VirtualList` | 공식 primitives에 이미 존재한다. 완성된 데이터 그리드와는 다르며 검색·행 식별·선택·복사는 앱의 책임이다 |
| 분석 차트 | `dioxus-charts` 우선 실증 | Rust/Dioxus 컴포넌트 API를 사용한다. 복잡한 시간축·행동/로그 선택 연결은 기능 검증이 필요하며, 부족해도 직접 JS 연결로 우회하지 않는다 |
| 전역 상태 | Dioxus의 Signal/Store/Context 우선 | Host와 선택 상태를 중복하는 별도 상태관리 패키지는 기본 요구사항이 아니다 |
| 비동기 실행 | Dioxus의 task/hook API와 기존 비동기 `HostHandle` | UI에서 직접 await하고 결과를 상태에 반영한다. Host용 스레드·명령 큐를 중복 생성하지 않는다. 라이브러리는 Tokio runtime을 요구하지 않으므로 별도 runtime은 실제 도입 API가 요구할 때만 결정한다 |
| 오류 보고 | `miette` — 확정 | 애플리케이션의 `Report`/`Result`와 진단 문맥에 사용한다. 라이브러리의 타입 기반 오류 계약은 유지한다. CLI의 터미널 표현과 GUI의 번역 가능한 오류 표시는 각 출력 환경에 맞춘다 |
| CLI | `usage-rs` — 확정 | Cargo 패키지명은 `usage-rs`, Rust 경로는 `usage_rs`다. child 모드까지 공통 파싱으로 분기하며 일반 CLI 작업은 기존 비동기 Host API를 사용한다 |
| 정규식 | managed 계층의 .NET `Regex` — 확정 | `regex`/`fancy-regex`를 앱의 직접 의존성으로 추가하지 않는다. 검색·검증 API는 구현이 필요하며, 타임아웃·취소·결과 수 제한과 매칭 위치 전달을 정의한다 |
| 진단 로그 수집 | `tracing` + `tracing-subscriber` — 확정 | 기존 `tracing`을 재사용하고 애플리케이션에서 수집·출력·수준별 필터를 설정한다. 사용자 대상 `miette` 오류 보고와 함께 사용한다 |
| 진단 로그 파일 | `tracing-appender` — 확정 | non-blocking writer와 파일 회전을 사용하고 정상 종료 시 worker guard를 해제해 남은 기록을 flush한다. 강제 종료 시 기록 보존은 보장하지 않는다. 진단 로그와 게임 원문 로그의 보존 정책은 구분한다 |
| 앱 경로 | ACT 관련 파일은 실행 파일 기준; 그 외 OS 표준 경로에는 `directories` | ACT 파일의 기존 상대 배치를 유지하고 Host에 실행 파일 기준으로 해석한 데이터 경로를 전달한다. `directories`는 ACT 관련 파일의 위치 결정에 사용하지 않는다 |
| 파일·폴더·링크 열기 | `open` — 확정 | 예를 들어 로그 폴더와 내보낸 파일을 OS 기본 앱으로 연다. 실행 지연을 UI 밖에서 처리하고 실패를 표시한다 |
| 날짜·시간대 | `jiff` — 확정 | 날짜 범위 입력, 시간대 변환과 표시를 담당한다. 기존 ACT 시간/식별자 계약은 유지하고 경과 시간 측정에는 단조 시계를 사용한다 |
| GUI 설정 저장 | TOML / `serde` + `toml_edit` — 확정 | 언어·테마·열 배치 등 앱 소유 설정을 `config.toml`에 저장한다. 주석·서식과 변경하지 않은 항목을 유지하며 저장 실패 시 기존 파일을 보존한다. `serde_json`은 이 설정 형식에 사용하지 않는다 |
| 국제화 | 초기부터 `dioxus-i18n` 사용 — 확정 | Fluent 기반이며 manifest상 Dioxus 0.7을 사용한다. UI 문구를 번역 리소스로 관리한다 |
| 보조 유틸리티 | 초기부터 `dioxus-sdk` 사용 — 확정 | 초기 화면에 필요한 SDK 기능을 활성화한다. 기존 Host 기능 및 설정 저장 책임과 중복되지 않게 선택한다 |
| 파일 열기/저장 | [`rfd`](https://github.com/PolyMeilex/rfd) | native 파일 대화상자 후보. import/export에서 사용하며 부모 창·취소·파일 경로 전달을 확인한다 |
| 클립보드 | [`arboard`](https://github.com/1Password/arboard) | native clipboard 후보. 전체/선택/검색 결과 등 복사 범위는 앱에서 명확하게 구분한다 |
| 독립 보조 창 | Dioxus Desktop 다중 창 API | 로그·타임라인 창을 모달로 대체하지 않는다. 각 창이 originating encounter/session을 보유하고 종료 시 자원을 정리한다. 정확한 API는 고정한 Desktop 버전에서 확인한다 |
| 플러그인 제공 UI | Windows child 소유 WinForms 설정 창과 기존 CEF 오버레이 | Dioxus는 설정 열기와 관리 진입점을 제공한다. 원본 UI를 재사용하고 STA·플러그인 소유권을 유지한다. 표시·선택 Host API와 실제 창 검증은 [구현 계획](gui-plan.md#plugin-provided-ui-implementation-plan)에 따른다 |

테이블/차트의 추천은 기능 적합성에 대한 판단이다. 라이브러리 자체가
aftercast의 세션 식별, 페이지 조회, 오래된 응답 폐기, 플러그인 정렬이나
개별 행동과 원문 로그의 연결을 구현해 주지는 않는다.

버전 기준은 안정판 0.7 계열이다. 공식 [v0.7.10 릴리스](https://github.com/DioxusLabs/dioxus/releases/tag/v0.7.10)를
확인했지만, 그 버전과 모든 위젯의 실제 Cargo 해석을 완료한 것은 아니다.
[v0.8.0-alpha.3 릴리스 노트](https://github.com/DioxusLabs/dioxus/releases/tag/v0.8.0-alpha.3)는
`dx components`의 TLS 실패와 기본 components branch의 Dioxus 0.7 의존성으로
인한 0.8 프로젝트 충돌을 알려준다. 알파 CLI와 안정판 컴포넌트를 혼합하지 않고,
구현 시 CLI·Dioxus·컴포넌트 커밋을 함께 고정한다.

## 세 조직에서 확인한 내용

| 후보 | 공식 근거로 확인한 내용 | aftercast 판단 |
| --- | --- | --- |
| [Dioxus Components](https://github.com/DioxusLabs/dioxus-components) | unstyled primitives 위의 styled 소스를 프로젝트로 가져오는 구조이며 `dx components add`를 안내한다 | Rust/UI에 부족한 동작을 보완할 때 primitives를 검토한다. 별도의 공식 styled 컴포넌트 테마를 함께 기본 도입하지 않는다 |
| [Components manifest](https://github.com/DioxusLabs/dioxus-components/blob/main/Cargo.toml), [primitives manifest](https://github.com/DioxusLabs/dioxus-components/blob/main/primitives/Cargo.toml) | 조사한 main은 Dioxus `0.7.8`; primitives의 manifest 버전은 `0.0.1`이다 | 저장소 main의 존재와 crates.io의 특정 배포물이 같은 코드를 포함한다는 주장을 구별해야 한다. 구현 시 선택한 배포/커밋과 Dioxus 버전을 함께 잠근다 |
| [primitives 공개 모듈](https://github.com/DioxusLabs/dioxus-components/blob/main/primitives/src/lib.rs) | context menu, dialog, calendar/date picker, drag-and-drop list, combobox, toast, virtual list 등이 있다 | 이름이 비슷한 커뮤니티 위젯을 먼저 추가하거나 같은 동작을 새로 만들 이유가 적다. 트리 전체 동작이나 데이터 그리드가 자동 제공된다고 해석하지 않는다 |
| [Rust/UI](https://github.com/rust-ui/ui/blob/main/README.md) | 독립 프로젝트이며 early-stage/experimental이라고 명시한다. crate 설치가 아닌 소스 복사 방식이고 Tailwind v4, `tw_merge`, Lucide용 `icons`를 사용한다 | Tailwind 기반 공통 컴포넌트의 기본 공급원으로 선택한다. 실험 단계이므로 가져올 컴포넌트별로 동작과 Desktop 호환성을 검증한다 |
| [Rust/UI manifest](https://github.com/rust-ui/ui/blob/main/Cargo.toml) | 조사한 main은 Dioxus `=0.7.9`를 고정하고 브라우저용 `web-sys` 의존성도 가진다 | 사이트가 Desktop 빌드를 제공한다는 것과 개별 컴포넌트가 native Rust Desktop에서 그대로 동작한다는 것은 별도다. 가져올 컴포넌트의 JS 접근 경로를 확인한다 |
| [Dioxus SDK](https://github.com/DioxusLabs/sdk), [manifest](https://github.com/DioxusLabs/sdk/blob/main/Cargo.toml) | time/storage/window/notification 등의 개별 crate를 제공한다. 개발 중이며 breaking change 가능성을 명시한다. main은 0.7 계열을 사용한다 | 초기부터 사용하며 필요한 기능을 활성화한다. notification 유틸리티는 ACT notification 이력/Show callback 구현과 동일하지 않다 |
| [dioxus-radio](https://github.com/dioxus-community/dioxus-radio/blob/main/Cargo.toml) | `0.7.1`, Dioxus `0.7`, topic 기반 구독 상태관리 | 선택적 후보. 기본 상태관리 위에 바로 추가할 필요는 없다 |
| [dioxus-i18n](https://github.com/dioxus-community/dioxus-i18n/blob/main/Cargo.toml) | manifest 버전 `0.6.0`이 Dioxus `0.7`과 Fluent `0.17`에 의존한다 | 초기 다국어 구성에 사용한다. crate 자체 버전만 보고 Dioxus 0.6 전용으로 오판하지 않는다 |

## 애플리케이션 오류 및 CLI

| 선택 | 공식 근거 및 적용 기준 |
| --- | --- |
| [`miette`](https://docs.rs/miette/latest/miette/) | `Diagnostic`과 `Report`/`Result`, 오류 문맥 및 진단 출력을 제공한다. `thiserror`와 함께 사용하며 라이브러리는 구체적 오류 타입을 반환한다. 터미널용 `fancy`가 필요하면 최상위 실행 크레이트에서 활성화한다 |
| [`usage-rs`](https://usage.jdx.dev/rust/) | Rust 구조체·enum에서 인자 파싱과 도움말 등을 생성하는 애플리케이션용 패키지다. `usage_rs` 경로로 사용하며 필요한 기능을 선택한다. CLI가 정상 도움말/파싱 오류로 종료하는 경로와 Host 실행 실패를 구분한다 |

시작 순서는 동기 `main` → 공통 CLI 파싱 → 실행 모드 분기다. child 모드는
`run_child_if_requested` 계열로 진입하고 완료 후 반환한다. 일반 앱 모드에서만
Dioxus·WebView·창·앱 비동기 런타임 등을 초기화한다. 기존 Host가 실행하는
`--aftercast-child` 인자 형태와 supervisor 인증은 유지한다.

현재 workspace의 Cargo manifest에는 `anyhow`, `miette`, `usage-rs`가 직접
의존성으로 선언되어 있지 않다. 이번 결정은 GUI/CLI 애플리케이션 구현의
선정 기준이며, 기존 라이브러리 오류 API나 내부 child 프로토콜을 일괄 변경하는
작업은 아니다. 도입할 배포 버전과 API는 구현 시 검증하고 고정한다.

## 운영 및 설정 라이브러리

아래 라이브러리는 모두 사용자 선정에 포함되었다. Cargo 의존성은 아직
추가하지 않았으며 GUI/CLI 실행 크레이트에서 도입한다.

| 공식 자료 | 역할과 구현 기준 |
| --- | --- |
| [tracing-subscriber](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/) | 애플리케이션의 구조화된 진단 이벤트를 수집하고 출력·필터를 구성한다 |
| [tracing-appender](https://docs.rs/tracing-appender/latest/tracing_appender/), [WorkerGuard](https://docs.rs/tracing-appender/latest/tracing_appender/non_blocking/struct.WorkerGuard.html) | 전용 기록 스레드의 non-blocking writer와 rolling file appender를 조합한다. rolling appender만으로는 비동기 기록이 되지 않는다. guard를 종료 시점까지 보유하되 해제 시의 flush 대기로 UI를 막지 않는다. 버퍼 포화 시 유실/대기 정책을 명시하고 강제 종료 시 남은 기록 보존을 보장하지 않는다 |
| [directories](https://docs.rs/directories/latest/directories/) | ACT 관련 파일 이외에 OS 표준 경로가 필요한 용도로 사용한다. ACT 설정·플러그인·데이터의 경로를 `ProjectDirs`로 결정하지 않는다 |
| [open](https://docs.rs/open/latest/open/) | 파일·폴더·URL을 OS 기본 앱으로 연다. helper 실행이 지연될 수 있으므로 UI 실행기에서 즉시 완료된다고 가정하지 않는다 |
| [jiff](https://docs.rs/jiff/latest/jiff/) | 날짜·시간대 계산과 표시를 처리한다. 로그의 원래 순서/식별자를 표시용 시간 변환으로 대체하지 않는다 |
| [serde](https://docs.rs/serde/latest/serde/), [toml_edit](https://docs.rs/toml_edit/latest/toml_edit/) | `toml_edit`의 `serde` 기능을 활성화해 앱 설정을 읽고, 편집 가능한 TOML 문서의 변경 항목만 갱신한다. 구조체 전체 재직렬화로 주석·알 수 없는 항목을 지우지 않는다. 기존 `serde_json` 의존성의 다른 용도는 유지한다 |

### aftercast TOML 설정

| 항목 | 구현 기준 |
| --- | --- |
| 파일 | 실행 파일 옆의 `config.toml`. 언어·테마 등 aftercast가 소유하는 설정만 포함한다 |
| 형식과 버전 | 최상위 `schema_version`을 두고 읽기 시 지원 여부와 설정 값을 검증한다. 지원하지 않는 버전이나 잘못된 파일은 진단하고 자동 덮어쓰지 않는다 |
| 편집 | `toml_edit`의 편집 가능한 문서를 유지하고 GUI에서 변경한 항목만 갱신한다. 주석·서식과 변경하지 않은 항목을 보존하며, 라이브러리의 서식 보존 한계도 확인한다 |
| 저장 | 완성한 내용을 같은 디렉터리의 임시 파일에 쓴 뒤 플랫폼에 맞는 원자적 교체로 저장한다. 실패 시 기존 파일을 보존하고 오류를 표시한다. 파일 I/O는 UI 실행기를 막지 않는다 |
| 기존 호환 설정 | ACT·플러그인의 XML 등 기존 형식을 TOML로 변환하거나 중복 저장하지 않는다 |

예시 스키마는 다음과 같다. 실제 키와 기본값은 설정 구현 시 확정한다.

```toml
schema_version = 1

[ui]
language = "ko-KR"
theme = "dark"

[logging]
level = "info"
```

### ACT 파일 배치

| 항목 | 기준 |
| --- | --- |
| 기준 디렉터리 | 실행 중인 애플리케이션 바이너리가 있는 디렉터리. 바로가기의 시작 위치나 터미널의 현재 작업 디렉터리에 의존하지 않는다 |
| ACT 관련 파일 | 실행 파일 옆과 기존 하위 디렉터리 배치를 사용한다. 기존 파일명·참조 관계를 보존하며 OS 표준 앱 데이터 폴더로 자동 이전하지 않는다 |
| 부모/child 일관성 | 부모에서 실행 파일 기준 경로를 해석해 Host 설정으로 전달하고 child에서도 같은 위치를 사용한다 |
| 쓰기 실패 | 오류와 대상 경로를 표시한다. OS 표준 경로로 조용히 우회하여 서로 다른 상태를 만들지 않는다 |
| 외부 입력 | 사용자가 선택한 원본 로그·플러그인 등 외부 파일을 이 정책만으로 자동 이동하거나 복사하지 않는다 |

## 테이블과 가상화

| 후보 | 확인한 기능 | 선택할 때 확인할 항목 |
| --- | --- | --- |
| [Tabulator](https://www.tabulator.info/docs/6.x/virtual-dom/) — 제외 | JS 기반 그리드로 가상화와 여러 표 기능을 제공한다 | 앱의 직접 JS 연결이 필요하므로 대체 후보에서도 제외한다 |
| [Chorale](https://github.com/zernst3/rust-chorale), [배포 manifest](https://docs.rs/crate/chorale-dioxus/0.2.3/source/Cargo.toml.orig) | 0.2.3, Dioxus 0.7. 프레임워크 독립 core와 Dioxus adapter; 정렬/필터/선택/가상화/열 조작 기능을 제공한다 | 기능은 유망하지만 신생 프로젝트다. Desktop에서 스크롤·키보드·행 활성화·열 조작·동적 갱신을 우선 실증한다. 기능 수만으로 운영 성숙도를 단정하지 않는다 |
| [공식 VirtualList](https://github.com/DioxusLabs/dioxus-components/blob/main/primitives/src/virtual_list.rs) | 동적 행 높이, 추정 크기, 가시 영역 버퍼를 지원한다. `document::eval` 메시지로 스크롤을 받는다 | 대량 로그 목록에 맞는 후보. 데이터 공급/페이지 관리, 특정 로그로 이동, 검색 결과 선택과 복사를 별도로 확인한다 |

예를 들어 encounter의 combatant 표와 개별 action 표는 같은 엔진에 서로 다른
열 정의를 주되, 행 활성화 결과는 앱이 식별자로 처리해야 한다. 화면에 보이는
행 번호나 표시 이름을 Host의 정체성으로 사용하지 않는다. 가상화는 화면의
DOM 수를 줄이는 기능이며, 전체 기록을 Rust/JS 메모리에 무제한 로드해도 된다는
뜻이 아니다. 로드된 한 페이지만 정렬하는 것과 전체 조회 결과를 정렬하는 것도
다르다. Host의 bounded query와 정렬 범위를 먼저 정하고, 일부 페이지만 받은
상태에서 전체 결과가 정렬된 것처럼 표시하지 않는다.

## 차트와 아이콘 대안

| 후보 | 확인 내용 | 판단 |
| --- | --- | --- |
| [Apache ECharts 이벤트](https://echarts.apache.org/handbook/en/concepts/event/) — 제외 | click, dblclick, contextmenu 및 datazoom 등의 이벤트와 데이터 인덱스를 전달한다 | 앱의 직접 JS 연결이 필요하므로 제외한다 |
| [Charming](https://github.com/yuankunzhang/charming/blob/main/README.md) — Desktop 차트 후보에서 제외 | ECharts 기반 Rust 차트 모델과 HTML/Image/WASM renderer가 있다. Image renderer는 SSR/내장 Deno, WASM renderer는 WASM 실행용이다 | Rust API가 있어도 native Dioxus Desktop에서 필요한 차트 연결을 자동 해결하지 않는다. ECharts 우회 도입에 사용하지 않는다 |
| [dioxus-charts](https://github.com/dioxus-community/dioxus-charts/blob/main/README.md), [manifest](https://github.com/dioxus-community/dioxus-charts/blob/main/Cargo.toml) | SVG 기반 bar/line/pie; 조사한 main은 crate 0.4.0/Dioxus 0.7이다. README 예제의 0.3은 main manifest와 다르다 | 차트 기본 후보로 우선 실증한다. 복잡한 시간축 분석까지 이 패키지로 해결된다고 미리 확정하지 않는다 |
| [dioxus-icons](https://github.com/DioxusLabs/dioxus-icons), [manifest](https://github.com/DioxusLabs/dioxus-icons/blob/main/Cargo.toml) | Lucide 컴포넌트, Dioxus 0.7.7부터의 0.7.x 지원, 아이콘별 컴포넌트. crate 라이선스는 MIT AND ISC | Rust/UI의 `icons`가 요구사항에 맞지 않을 때의 대안 |
| [lucide-dioxus](https://lucide.rustforweb.org/frameworks/dioxus.html), [workspace manifest](https://github.com/RustForWeb/lucide/blob/main/Cargo.toml) | RustForWeb의 별도 Lucide 구현, Dioxus 0.7; 선택적 아이콘 카테고리 기능 | Rust/UI의 `icons` API나 빌드 비용이 맞지 않을 때의 대안 |
| [rust-ui/icons manifest](https://github.com/rust-ui/icons/blob/main/Cargo.toml) | `icons` 0.18.1, Dioxus 0.7, `tw_merge` 의존 | Rust/UI 컴포넌트와 일치하는 기본 아이콘으로 선택한다 |
| [dioxus-free-icons](https://github.com/dioxus-community/dioxus-free-icons), [workspace manifest](https://github.com/dioxus-community/dioxus-free-icons/blob/main/Cargo.toml) | 다양한 아이콘 세트, Dioxus 0.7. 원본 세트별 라이선스가 다르다 | Lucide 하나로 충족하면 추가하지 않는다. 특정 세트가 필요할 때 선택 |

## 구현 기준

GUI 테스트 도구와 도입 순서, 회귀 시나리오 및 검증 한계는
[GUI 테스트 계획](gui-plan.md#gui-test-plan)에 정리했다. Rust 테스트와
Windows 실제 WebView2 앱의 Playwright 자동화를 조합하며, Desktop 연결은
구현 초기에 실증한다. Playwright는 개발·테스트 도구이며 앱의 JS 연결이 아니다.

위의 JS 연결 금지, 초기 i18n/SDK 사용, Tailwind 기반 구성, 초기 비동기 구현, `miette`/`usage-rs` 사용 및 .NET 정규식 엔진 재사용은 사용자 확정 조건이다.
운영·설정용 `tracing-subscriber`·`tracing-appender`·`directories`·`open`·`jiff`와 TOML 설정용 `serde`·`toml_edit` 사용도 확정하였다.
ACT 관련 파일은 OS 표준 경로 대신 실행 파일 옆에 배치하는 것으로 확정하였다.
확정 선택은 구현 시 해당 목적의 의존성을 추가·사용하는 기준이다.
`우선 실증`으로 표시한 후보는 필요한 기능과 Desktop 호환성을 검증한 뒤 채택한다.
이 문서가 저장소의 `AGENTS.md`를 수정한 것은 아니다.

| 원칙 | 실행 예 |
| --- | --- |
| 선정된 라이브러리를 쓰는 것이 기본이다 | 필요한 메뉴·포커스 제어·테이블 가상화를 의존성 수를 줄이려는 이유만으로 직접 재구현하지 않는다 |
| 선정 범주와 초기 필수 구성을 따른다 | i18n과 SDK는 GUI 초기 구성부터 사용한다. 나머지 후보는 해당 기능 구현 시 도입하며 사용하지 않는 기능을 모두 미리 설치하지는 않는다 |
| Tailwind 기반 기본 조합을 유지한다 | Rust/UI·Tailwind v4·`tw_merge`·Lucide `icons`를 사용한다. 예를 들어 Rust/UI에 없는 가상 목록은 공식 primitives를 재사용하되 앱의 Tailwind 테마에 맞춘다 |
| 비동기를 후속 리팩터링으로 미루지 않는다 | 예를 들어 encounter 선택 → 비동기 조회 → session/selection 확인 → 상태 반영으로 구현한다. 블로킹 API나 무거운 계산은 플랫폼의 스레드 제약을 지키며 제한된 작업 실행 경로로 분리한다 |
| 선정한 오류·CLI 라이브러리를 사용한다 | 애플리케이션 오류 보고에는 `miette`, CLI에는 `usage-rs`를 사용한다. child 모드도 공통 CLI 파싱에 포함하고 UI·런타임 초기화 전에 분기한다. 기존 구체적 라이브러리 오류와 supervisor 인증을 보존한다 |
| 정규식 문법과 실행 엔진을 통일한다 | 로그 정규식 검색과 트리거 검증·실행은 .NET에서 처리한다. 예를 들어 GUI에서 패턴을 다른 Rust 엔진으로 사전 검증하지 않는다. 매칭 위치는 .NET의 UTF-16 인덱스와 Rust의 UTF-8 바이트 인덱스를 구분해 전달한다 |
| 대체에는 구체적 근거가 필요하다 | 버전 충돌, Desktop 미지원, 필요한 기능 부재 또는 재현한 성능 문제가 있으면 근거를 남기고 대체한다 |
| 복사한 컴포넌트도 외부 코드로 추적한다 | 원본 URL·고정 커밋·라이선스·로컬 수정 이유를 남긴다. Cargo dependency가 아니라는 이유로 유지보수 비용을 무시하지 않는다 |
| 직접 JS 연결을 대안으로 사용하지 않는다 | Rust/Dioxus API로 구현한다. 라이브러리가 부족하면 한계를 기록하고 조건에 맞는 대안을 찾으며 직접 `document::eval` 또는 외부 JS 위젯 연결을 추가하지 않는다 |
| 도입 검증은 실제 위험에 맞춘다 | 빈 창 성공에 그치지 않고 한국어 IME·DPI·키보드/포커스·대량 행·창 두 개의 독립 문맥을 확인한다 |

아직 확인하지 않은 사항: 최종 Cargo 해석과 release별 API 차이, 실제 Windows/Linux
실행, 패키징, 성능 및 접근성. 버전과 Desktop 적합성은 구현 시작 시 고정 조합으로
검증해야 한다.
