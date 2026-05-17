# spctr ROADMAP

## 現在地（2026-05-03 時点）

完成しているもの：

- **JSON superset** な構文。任意の JSON ドキュメントが valid spctr
- **HM 風型推論**。注釈ゼロで多相型が flow する。typeck は per-node 型を `node_types: HashMap<usize, Type>` に記録し JIT が monomorphization に使う
- **tree-walker インタプリタ**（`src/interp.rs`）が主軸。fib(25) ≒ 37ms
- **stdlib（全 Rust 実装、全 typed）**：`List`(10) / `String`(6) / `Number`(10) / `import`
- **resolver パス**で `Variable(VarRef)` を `BindRef(depth, slot)` に解決
- **ariadne** によるスパン付きエラー表示
- **rustyline REPL**（引数なし or `--repl`）
- **insta** スナップショットテスト 24個 + JIT スモークテスト 34個
- **criterion** ベンチ
- **`import("./path")`** によるユーザライブラリ
- **`--type`** で型を表示、**`--check`** で型エラーのみ確認
- **Cranelift JIT (Phase 3f)**（`src/jit.rs`、`--jit` フラグ）：数値 + first-class function + closure + polymorphic multi-instance + record + top-level non-function bindings + list + string + stdlib + `&&`/`||` 短絡 + null + ImmediateBlock + **任意の戻り値の display**（`spctr_print` 経由で record/list/string も tree-walker と同じ出力）。`run()` は数値専用、`run_with_display()` は任意の型を JIT 内で format & print。fib(38) tree-walker 18.4s → JIT 0.31s ≒ 60倍

## やらないと決めたこと

- **バイトコード VM の復活**：tree-walker と将来の JIT/WASM があれば中間 IR は不要
- **null の Option化**：JSON 互換のため `null` は singleton type のまま
- **Iterator stdlib の再導入**：List builtin で代替済み（必要なら遅延列を別形で）

---

## 次の方向（独立、好きな順で）

### (δ) 言語機能拡張

- パターンマッチ：`match expr { pat => ... }`
- `?.` (optional chaining)：`obj?.field?.method()`
- null との型合流：union or option type
- ✅ 文字列補間：`"hello ${name}"` — done 2026-05-17。`${expr}` 部分は `string` 型必須（typeck で unify、auto-stringify は Phase 4 NaN-boxing 待ち）。lexer は `${` でスキャンを分割して `StrBegin/StrLit/InterpOpen/.../InterpClose/StrEnd` シーケンスを emit、plain string は単一 `Token::Str(s)` のまま。JIT は `spctr_str_concat` で左→右に逐次 concat。

**コスト**：中〜大。パターンマッチは特に大物
**効果**：実用度が一段上がる

### (ε) Row polymorphism

`(r) => r.x + 1 : forall ρ. {x: number | ρ} → number` のように「x フィールドを持つ何か」を型として扱える。

**コスト**：大。HM の本筋を一段深める
**効果**：spctr の個性（Block-as-record）が型でも活きる。Iterator 風の構造を再導入する場合は事実上必須

### (β) Cranelift JIT

AST → Cranelift IR 直行で native コード生成。tree-walker は reference impl として残す。

**段階**：
1. ✅ 数値演算のみ JIT（fib が JIT で動く）— done 2026-05-03
2. ✅ クロージャ + ヒープ helper（runtime helper を `extern "C"` で呼ぶ）— done 2026-05-03
2.5. ✅ Polymorphic multi-instance — done 2026-05-03
3a. ✅ Records（block + field access）— done 2026-05-03
3d. ✅ Top-level non-function bindings（`add5: make_adder(5)`）— done 2026-05-03
3b. ✅ Lists（element ty ごとに monomorphize、`[len: u32][slot: 8B]*n` layout）— done 2026-05-03
3c. ✅ Strings（leak した `[len: u32][_pad: u32][bytes]` 静的バッファ＋`spctr_str_eq` 構造比較。`==` / `!=` を IR type で dispatch）— done 2026-05-03
3e. ✅ stdlib 連携：List/String/Number 全 26 関数 — done 2026-05-03
3f. ✅ `&&` / `||` 短絡、null、ImmediateBlock、任意戻り値 display（B path） — done 2026-05-03
3g. ✅ list の構造比較（`emit_value_eq` で要素型を辿る再帰 lower）、record/closure 比較は tree-walker と同じく常に false に固定。record-by-string indexing はリテラル限定で parser desugar 経由で既に動作することを確認しテストで固定 — done 2026-05-17
3h. ✅ 前方参照の緩和。top-level Phase B を「Value 評価 → Function captures populate」の2 段に分け、function→later-value forward ref が動くように。block も同等の Phase 1/2/3 構造（function literal は Phase 1 で alloc + sibling cap を deferred、Phase 2 で value を source order に評価 + deferred cap を機会的に populate、Phase 3 で残り cap = 真サイクルを reject）。`BlockFrame.populated` を `Vec<bool>` に変更。これで block 内 mutual recursion と function→later-value forward ref が動く。value→value forward ref と「value が後方 value を capture する関数を呼ぶ」ケースは silent-wrong だったのを compile-time error に格上げ — done 2026-05-17
3i. （未着手）import、性能 polishing
4. NaN-boxing にスイッチ（必要になったら）— Path A への切り替え選択肢として残す。null と他型の union、record の動的 string indexing（フィールド型異種の場合）、value→value forward ref、value-calls-function-with-later-cap はここで初めて意味を持つ

**Phase 3h までできること**：上記すべて + top-level/block での **function→later-value forward ref**（`add_n: (x) => x + n, n: 10, add_n(5)` が 15 を返す）、**block 内 mutual recursion**（`is_even` / `is_odd` が動く）。value→value forward ref と value-calls-function-with-later-cap は明示的なエラーで reject。  
**Phase 3h でできないこと**：import、value→value forward ref、value-calls-function-with-later-cap（後ろ 2 つは値タグ前提の Phase 4 で対応）。

**Closure layout**: `[fn_ptr: 8][n_caps: 4][_pad: 4][cap_slot_0: 8][cap_slot_1: 8]...`。`spctr_alloc_closure(fn_ptr, n_caps)` でヒープから確保（leak）。すべての関数は `(closure_ptr: i64, args...) -> ret` の ABI。  
**Record layout**: `[slot_0: 8][slot_1: 8]...`。`spctr_alloc_record(n_slots)` で確保。field offset = `8 * field_index`、field type は `Type::Record` の宣言順。  
**List layout**: `[length: u32][_pad: u32][slot: 8B]*n`。`spctr_alloc_list(n)` で確保。indexing は `8 + 8 * idx` offset（length header をスキップ）。  
**String layout**: `[length: u32][_pad: u32][bytes]`。リテラルは JIT compile 時に `Box::leak` で静的領域に置いてポインタ定数を埋め込む。等価比較は `spctr_str_eq` で長さチェック→バイト比較。  
**stdlib dispatch**: `Call(Access(Variable(M), field), args)` パターンで `M` が `List`/`String`/`Number`（root frame slot 0/1/2）かを `distance_to_root(env)` で判定。マッチしたら intrinsic（Cranelift 直命令）か runtime helper か inline ループに dispatch。`Type::Module` は capture 対象から除外（statically resolved）。  
**Inline loops (List.map/filter/reduce)**: 入出力 element type を typeck から取り、closure を `call_indirect` で呼ぶループ block を JIT で構築。filter は worst-case 確保→末尾で length patch。  
**Display path**: `run_with_display()` 経由で `Compiler.display=true`、main の body 値を `emit_display(val, ty, ...)` 再帰関数で format して `spctr_print` に流し込む。bool/list は branch/loop block を JIT で構築、record は alphabetical sort で interp と同じ出力に。`__spctr_main` の戻り値は常に f64 で、display モードでは sentinel 0.0。`run()` は今まで通り数値専用（テスト用）、`run_with_display()` を main.rs から呼ぶ。  
**Top-level instances**: `TopInstance.kind = Function | Value(IrType)`。Phase A で全 function closure pre-alloc + 全 CVar declare。Phase B で source order に function captures populate / value body 評価 def_var。後ろの value への forward ref（function capture / value body 双方）を compile time reject。  
**Monomorphization**: typeck の per-node types を使い、worklist BFS で全 function instance を発見。main の body と全 non-function binding body から seed → 各 instance の body を所属 subst で scan → 新たな use を発見 → 不動点。`FuncKey = (expr_ptr, mono_ty_str)` で `funcs` をキー化、`Capture` も `mono_ty_str` を保持して allocation 時に正しい `TopInstance` に dispatch。  
**Block frames**: `CompileEnv.block_frames: Vec<BlockFrame>` で record_ptr + populated count を innermost-first で stack。bref.depth が block_frames に届く間は record から load、超えた分だけ底の Function/Main に届く。collect_captures / collect_uses_in も Block / ImmediateBlock で layer +1。

**コスト**：とても大
**効果**：性能が二桁オーダで伸びる（実測）。学習として圧倒的に濃い

### (γ) WASM 出力

`wasm-encoder` で `.wasm` を吐く。stack-based なので現状の Cmd 列っぽい中間表現と相性◎。

**コスト**：中〜大
**効果**：ブラウザで動く spctr。JIT より「同じ意味論を別実装」の比較教材として面白い

### (ζ) 周辺の磨き込み

- 型変数を `α/β/γ` に rename して表示（今は `?1048576` などの生 ID）
- 64MB stack hack を消す（TCO or iterative trampoline）
- ベンチ充実（より多角的な性能測定）
- エラーメッセージの polish

**コスト**：小〜中
**効果**：使い心地と健全性

---

## 直近のメモ・注意点

### chumsky の型爆発

parser の precedence 層を増やすたびに rustc が秒単位→分単位で詰まる。**`.boxed()` で各層を型消去** している。新しい precedence 層を追加するときは必ず `.boxed()` を入れること。これを外すと9分ビルドに戻る。

### 64MB stack hack

`src/main.rs` で interp スレッドを `thread::Builder::stack_size(64MB)` で起動している。tree-walker の再帰がそのまま Rust の呼び出しスタックを食うため。TCO を入れるか、iterative trampoline 化するまでは必要。

`.cargo/config.toml` でテスト時の `RUST_MIN_STACK` も上げてある。

### lasso interner と TypeVar の衝突

stdlib の builtin scheme は `TypeVar(0)`, `TypeVar(1)` などの低 ID を量化変数に使う。typeck の inferer は fresh var を `INFERER_VAR_START = 1<<20` から始めて衝突を回避している（`src/typeck.rs`）。これを下げると instantiate 時に subst が自己ループして apply が無限再帰する。

### 性能伸びしろ（Tier 2 まだ未着手）

- SmallVec for args（小さな関数呼び出しの heap alloc 排除）
- AST arena (bumpalo) でキャッシュ局所性
- TCO

これらは「tree-walker としての完成度をさらに高める」方向。JIT に行くなら飛ばしても良い。

---

## ファイル配置メモ

```
src/
├── ast.rs           AST 定義（Spanned<T>, VarRef, BindRef）
├── diag.rs          Diagnostic + ariadne 表示
├── interp.rs        tree-walker
├── lexer.rs         logos lexer
├── lib.rs           lib crate root
├── main.rs          bin entry: file/-c/REPL
├── parser.rs        chumsky parser（.boxed() 必須）
├── resolver.rs      AST → 解決済みAST
├── symbol.rs        lasso ベースの interner
├── types.rs         Type, Scheme, Subst
├── typeck.rs        HM 推論
└── stdlib/
    ├── imports.rs
    ├── list.rs
    ├── number.rs
    ├── string.rs
    └── mod.rs

examples/
├── fib.spc
├── fizzbuzz.spc
├── math.spc
├── middle.spc
├── use_math.spc
└── util.spc

tests/snapshots.rs   24 insta スナップショットテスト
benches/interp.rs    criterion ベンチ
```
