# rumble

IMAP の良さをそのまま享受する、**メールをローカルに保存しない**軽量メーラー。
[Tauri](https://tauri.app/) + Rust + Svelte 製。

## コンセプト

IMAP ではメールの正本はサーバーにあります。それなのに多くのメーラーは全メールを
手元に複製し、使うほど肥大化していきます。rumble はそれをやめました。

| | 一般的なメーラー | rumble |
|---|---|---|
| メール本体 | 全件をローカル DB に同期 | **保存しない**。開いたときにサーバーから取得 |
| 一覧 | ローカル DB から表示 | 表示するぶんだけサーバーから取得 (50 件ずつ) |
| 検索 | ローカルの索引 | サーバー側検索 (IMAP `SEARCH`) |
| 新着 | 定期的に全体を同期 | IMAP `IDLE` によるプッシュ通知 |
| 既読・スター・移動 | ローカルを変更して後で同期 | その場でサーバーへ反映 |
| ディスク使用量 | メール量に比例して増える | **増えない** |

rumble がディスクに書くのは次の 2 つだけです。

- アカウント設定 (ホスト名など) … 設定ディレクトリの `accounts.json`
- パスワード … OS のキーチェーン

開いたメールは体感速度のためにメモリ上へ一時的に保持しますが (上限 64 MB)、
アプリを終了すれば何も残りません。添付ファイルは、保存先を選んで「保存」したときだけ
ディスクに書き出されます。

## 機能

- 複数アカウント (IMAP + SMTP、SSL/TLS・STARTTLS)
- フォルダ一覧 (受信トレイ・送信済みなどの役割を自動判別、日本語フォルダ名に対応)
- メール一覧 (スクロールで続きを取得)、サーバー側検索
- 本文表示 (テキスト / HTML)。HTML はサニタイズし、外部画像は既定でブロック
- 既読・未読、スター、アーカイブ、フォルダ移動、削除
- 新規作成・返信・全員に返信・転送、ファイル添付
- 添付ファイルの保存
- IDLE による受信トレイのリアルタイム更新
- ライト / ダークテーマ (OS の設定に追従)

キーボード: `j`/`k` または `↑`/`↓` で移動、`r` で返信 (`Shift+r` で全員に返信)、
`Delete` で削除、`⌘/Ctrl+N` で新規作成、`⌘/Ctrl+Enter` で送信。

## 使い方

必要なもの: [Rust](https://rustup.rs/)、Node.js、[pnpm](https://pnpm.io/)、
および [Tauri の前提パッケージ](https://tauri.app/start/prerequisites/)。

```sh
pnpm install
pnpm tauri dev      # 開発モードで起動
pnpm tauri build    # 配布用にビルド
```

初回起動時にアカウント追加画面が開きます。Gmail・iCloud などは通常のパスワードでは
ログインできないので、各サービスで **アプリ パスワード** を発行して入力してください。

## 開発

```sh
cargo test --workspace     # Rust のテスト
cargo clippy --workspace   # lint
pnpm check                 # フロントエンドの型チェック
```

本物の IMAP サーバー (Dovecot) を相手にした統合テストもあります。

```sh
docker run -d --rm --name rumble-test-imap -e USER_PASSWORD=password \
    -p 31993:31993 dovecot/dovecot:latest
cargo test -p rumble-core --test imap_server -- --ignored
```

コードの構成と設計判断は [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) にまとめています。

## いまできないこと

- OAuth2 認証 (Outlook.com / Microsoft 365 など、パスワード認証を廃止したサービスは使えません)
- 下書きの保存 (書きかけのメールは作成画面を閉じると消えます)
- 複数メールの一括操作、スレッド表示
- HTML メールの作成、転送時の添付ファイル引き継ぎ
- 複数フォルダの横断検索
- 添付だけを除いた部分取得 (メールを開くと添付ごとメモリへ取得します)
