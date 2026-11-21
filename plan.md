# Rust migration plan: voice-ai (Python)

1 packages, 43 source files (7705 lines), 25 test files, 0 UI files, grouped into 13 units.

| Ready | Caution | Blocked | Keep in source language |
| ---: | ---: | ---: | ---: |
| 8 | 5 | 0 | 0 |

## Start here

Best first ports: safe to port (readiness) and worth porting (loops and string work, tests, size).

| # | Unit | Lines | Priority | Readiness | Tests | Bridge | Main risks |
| ---: | --- | ---: | ---: | ---: | ---: | --- | --- |
| 1 | `training/tts/` | 94 | 74 | 100 | 0 | PyO3 | - |
| 2 | `training/` | 135 | 73 | 94 | 0 | PyO3 | String offsets and slicing (2) |
| 3 | `src/ai_assistant/ipc/` | 177 | 73 | 92 | 2 | PyO3 | String offsets and slicing (1); Concurrency (2); Exceptions (13) |
| 4 | `training/whisper/` | 98 | 72 | 97 | 0 | PyO3 | Exceptions (1) |
| 5 | `src/ai_assistant/ai/` | 455 | 61 | 52 | 4 | PyO3 | String offsets and slicing (10); Regexes built at runtime (5); Hash iteration order (2) |
| 6 | `src/ai_assistant/speech/` | 351 | 60 | 58 | 4 | PyO3 | String offsets and slicing (5); Regexes built at runtime (3); Dynamic typing (2) |
| 7 | `src/ai_assistant/` | 840 | 55 | 65 | 8 | PyO3 | Hash iteration order (1); Integer width and overflow (1); Concurrency (1) |

### `training/tts/`

94 lines in 1 files, wave 0, priority 74, readiness 100, bridge **PyO3**.

- Leaf: imports no other unit.

### `training/`

135 lines in 1 files, wave 0, priority 73, readiness 94, bridge **PyO3**.

- Leaf: imports no other unit.
- String offsets and slicing x2: `training/generate_dataset.py:115`, `training/generate_dataset.py:125`

### `src/ai_assistant/ipc/`

177 lines in 3 files, wave 0, priority 73, readiness 92, bridge **PyO3**.

- Import cycle with src/ai_assistant/tools/, src/ai_assistant/speech/, src/ai_assistant/audio/, src/ai_assistant/core/, src/ai_assistant/ai/, src/ai_assistant/: port them together.
- 2 test files import it directly.
- String offsets and slicing x1: `src/ai_assistant/ipc/server.py:137`
- Concurrency x2: `src/ai_assistant/ipc/server.py:141`, `src/ai_assistant/ipc/server.py:163`
- Exceptions x13: `src/ai_assistant/ipc/client.py:14`, `src/ai_assistant/ipc/server.py:34`, `src/ai_assistant/ipc/server.py:40`

### `training/whisper/`

98 lines in 1 files, wave 0, priority 72, readiness 97, bridge **PyO3**.

- 1 PyPI dependencies not in the crate table: check them by hand.
- Leaf: imports no other unit.
- Exceptions x1: `training/whisper/record_dataset.py:95`

### `src/ai_assistant/ai/`

455 lines in 4 files, wave 0, priority 61, readiness 52, bridge **PyO3**.

- 1 PyPI dependencies not in the crate table: check them by hand.
- Import cycle with src/ai_assistant/ipc/, src/ai_assistant/tools/, src/ai_assistant/speech/, src/ai_assistant/audio/, src/ai_assistant/core/, src/ai_assistant/: port them together.
- 4 test files import it directly.
- String offsets and slicing x10: `src/ai_assistant/ai/ollama_client.py:85`, `src/ai_assistant/ai/ollama_client.py:136`, `src/ai_assistant/ai/ollama_client.py:138`
- Regexes built at runtime x5: `src/ai_assistant/ai/model_router.py:63`, `src/ai_assistant/ai/model_router.py:72`, `src/ai_assistant/ai/ollama_client.py:241`
- Hash iteration order x2: `src/ai_assistant/ai/model_router.py:14`, `src/ai_assistant/ai/model_router.py:17`
- Dynamic typing x1: `src/ai_assistant/ai/ollama_client.py:130`
- Concurrency x1: `src/ai_assistant/ai/ollama_client.py:157`
- Exceptions x7: `src/ai_assistant/ai/model_router.py:11`, `src/ai_assistant/ai/ollama_client.py:102`, `src/ai_assistant/ai/ollama_client.py:118`

## Order by package

Packages come after the workspace packages they use. Inside a package, port by wave: a unit's imports are in earlier waves.

### 1. voice-ai (`.`, portable, 25 test files)

| Wave | Unit | Status | Lines | Readiness | Bridge | Main risks |
| ---: | --- | --- | ---: | ---: | --- | --- |
| 0 | `training/tts/` | ready | 94 | 100 | PyO3 | - |
| 0 | `training/whisper/` | ready | 98 | 97 | PyO3 | Exceptions (1) |
| 0 | `training/wakeword/` | caution | 83 | 96 | PyO3 | Integer width and overflow (1) |
| 0 | `training/` | ready | 135 | 94 | PyO3 | String offsets and slicing (2) |
| 0 | `src/ai_assistant/ipc/` | ready | 177 | 92 | PyO3 | String offsets and slicing (1); Concurrency (2) |
| 0 | `src/ai_assistant/` | ready | 840 | 65 | PyO3 | Hash iteration order (1); Integer width and overflow (1) |
| 0 | `src/ai_assistant/speech/` | ready | 351 | 58 | PyO3 | String offsets and slicing (5); Regexes built at runtime (3) |
| 0 | `src/ai_assistant/ai/` | ready | 455 | 52 | PyO3 | String offsets and slicing (10); Regexes built at runtime (5) |
| 0 | `src/ai_assistant/audio/` | caution | 265 | 45 | PyO3 | numpy array parameters (3); String offsets and slicing (1) |
| 0 | `src/ai_assistant/core/` | caution | 2165 | 0 | PyO3 | Regexes built at runtime (16); String offsets and slicing (9) |
| 0 | `src/ai_assistant/tools/` | caution | 2489 | 0 | PyO3 | Regexes built at runtime (28); String offsets and slicing (10) |
| 1 | `assistant.py` | ready | 132 | 100 | PyO3 | - |
| 1 | `benchmarks/` | caution | 421 | 86 | PyO3 | String offsets and slicing (2); Exceptions (1) |

## PyPI dependencies

| Package | Units | Rust | Fit | Note |
| --- | ---: | --- | --- | --- |
| `sounddevice` | 6 | ? | unknown |  |
| `faster-whisper` | 2 | ? | unknown |  |
| `ctranslate2` | 1 | ? | unknown |  |
| `nvidia` | 1 | ? | unknown |  |
| `openwakeword` | 1 | ? | unknown |  |
| `piper` | 1 | ? | unknown |  |
| `scipy` | 1 | ? | unknown |  |
| `numpy` | 7 | ndarray | partial | Broadcasting and dtype rules differ; check numerics. |
| `pil` | 1 | image | partial |  |
| `requests` | 4 | reqwest | good |  |
| `urllib3` | 1 | reqwest | good |  |

## Risk guide

- **String offsets and slicing** (High): JS indexes strings in UTF-16 code units, Rust in UTF-8 bytes. Offsets that cross the boundary must be converted, or non-ASCII text gets wrong ranges silently.
- **Regex features the `regex` crate lacks** (High): Lookaround, backreferences and the `y` flag are not in the `regex` crate. Use `fancy-regex` for those patterns, or rewrite them, and test them on non-ASCII text: `\w`, `\b` and case-insensitive matching differ between engines even where a pattern compiles. JS `/i` without `u` never matches a non-ASCII letter to an ASCII one, Rust `(?i)` does (`ſ` matches `s`, the Kelvin sign `K` matches `k`): write `(?i-u)` or compare with `eq_ignore_ascii_case`.
- **Regexes built at runtime** (High): Patterns come from data or config in JS syntax. Rust regex syntax differs, so user-visible behavior can change. Pin it with tests on real patterns.
- **Dynamic typing** (Medium): Values whose shape is checked at runtime need an explicit Rust enum or `serde_json::Value`.
- **Exceptions** (Low): Thrown errors become `Result`. Errors crossing the bridge must map back to the same JS error shape.
- **Class inheritance** (Medium): Class hierarchies become traits plus composition; `instanceof` checks become enums.
- **Number semantics** (Low): JS numbers are f64; bitwise ops truncate to i32; parseInt is lenient. Pick Rust integer types deliberately. Printing differs: `${n}` gives `1e+21`, `1e-7` and `0` for -0 where Rust's `{}` does not, so use `js::number_to_string`. `Math.round` rounds halves up (-2.5 gives -2), Rust's `round` away from zero: use `js::math_round`. `toFixed` rounds exact halves up ((2.5).toFixed(0) is "3"), Rust's `{:.0}` to even.
- **Integer width and overflow** (Medium): Python ints never overflow; C, Go and Java ints wrap or are undefined on overflow. Pick i64, i128 or a bignum crate deliberately, and use wrapping_* where the source wraps.
- **Division and modulo** (Medium): Python `//` and `%` floor toward negative infinity; Rust `/` and `%` truncate: use `div_euclid`/`rem_euclid` or explicit floor math. C division by zero is undefined.
- **Hash iteration order** (High): Set and map iteration order comes from the source language's hash function. A port cannot reproduce it: sort before output, or make the order part of the contract.
- **Concurrency** (Medium): Threads, goroutines and locks need a design in Rust (channels, Send/Sync), not a line-by-line translation; keep concurrency in the caller where possible.
- **numpy array parameters** (High): `record` passes JSON values, never numpy arrays, so a function that takes `np.ndarray` gets no valid cases on the unit path. Record the app instead (`app record`, `app replay`), or port the function together with a caller that takes and returns plain lists.
