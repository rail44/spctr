# spctr ROADMAP

## 現在地（2026-05-02 時点）

完成しているもの：

- **JSON superset** な構文。任意の JSON ドキュメントが valid spctr
- **HM 風型推論**。注釈ゼロで多相型が flow する
- **tree-walker インタプリタ**（`src/interp.rs`）が主軸。fib(25) ≒ 37ms
- **stdlib（全 Rust 実装、全 typed）**：`List`(10) / `String`(6) / `Number`(10) / `import`
- **resolver パス**で `Variable(VarRef)` を `BindRef(depth, slot)` に解決
- **ariadne** によるスパン付きエラー表示
- **rustyline REPL**（引数なし or `--repl`）
- **insta** スナップショットテスト 24個
- **criterion** ベンチ
- **`import("./path")`** によるユーザライブラリ
- **`--type`** で型を表示、**`--check`** で型エラーのみ確認

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
- 文字列補間：`"hello ${name}"`

**コスト**：中〜大。パターンマッチは特に大物
**効果**：実用度が一段上がる

### (ε) Row polymorphism

`(r) => r.x + 1 : forall ρ. {x: number | ρ} → number` のように「x フィールドを持つ何か」を型として扱える。

**コスト**：大。HM の本筋を一段深める
**効果**：spctr の個性（Block-as-record）が型でも活きる。Iterator 風の構造を再導入する場合は事実上必須

### (β) Cranelift JIT

AST → Cranelift IR 直行で native コード生成。tree-walker は reference impl として残す。

**段階**：
1. 数値演算のみ JIT（fib が JIT で動く）
2. クロージャ + ヒープ helper（runtime helper を `extern "C"` で呼ぶ）
3. 値表現（NaN-boxing or タグ付き構造体）
4. records / lists / strings

**コスト**：とても大
**効果**：性能が二桁オーダで伸びる。学習として圧倒的に濃い

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
