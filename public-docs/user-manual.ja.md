# PKV Sync ユーザーマニュアル

[English](./user-manual.md) | [简体中文](./user-manual.zh-CN.md) | [繁體中文](./user-manual.zh-Hant.md) | 日本語 | [한국어](./user-manual.ko.md)

この文書は機械翻訳による初版です。公開前にネイティブ話者によるレビューを推奨します。

このマニュアルは、既存の PKV Sync サーバーに接続する Obsidian ユーザー向けです。始める前に、サーバー管理者からサーバー共有 URL とアカウントまたは招待コードを入手してください。

## プラグインの手動インストール

1. 対応する GitHub release から `pkv-sync-plugin.zip` をダウンロードします。
2. アーカイブを vault に展開します。

   ```text
   <vault>/.obsidian/plugins/pkv-sync/
   ```

3. Obsidian で community plugins を有効にします。
4. **PKV Sync** を有効にします。

展開後のディレクトリには `main.js`、`manifest.json`、`styles.css` が含まれている必要があります。

## Plugin Updates

PKV Sync の settings page には **Updates** セクションがあります。既定では、プラグインは接続先の PKV Sync server にある bundled plugin version を確認します。これは self-hosted deployment では推奨の source です。server を upgrade すると、対応する plugin assets も公開されます。`public_host` が設定されている場合、plugin asset URLs はその外部 host に固定されます。必要に応じて update source を GitHub releases に切り替えられます。

更新がある場合、**Update now** は `main.js`、`manifest.json`、存在する場合は `styles.css` を download し、SHA-256 を verify して plugin files に書き込み、Obsidian の reload を促します。command palette にも **PKV Sync: Check for PKV Sync plugin updates** があります。

## サーバーに接続する

サーバー共有 URL は通常次のような形式です。

```text
https://sync.example.com/k_xxx/
```

**Settings -> PKV Sync** を開き、共有 URL を貼り付けて **Connect** をクリックします。deployment key が URL に埋め込まれている場合、プラグインが自動的に入力します。

誤ったサーバーを入力した場合、または別のセルフホストサーバーへ移動する必要がある場合は、ログイン画面の **Change server** を使うと、プラグインを再インストールせずにサーバー設定へ戻れます。

## ログインまたは登録

登録の動作はサーバーの実行時設定に依存します。

- **Disabled**: 管理者がアカウントを作成する必要があります。
- **Invite only**: 管理者から提供された招待コードを入力します。
- **Open**: 直接アカウントを作成できます。

ログイン後、既存のリモート vault を選択するか新しいリモート vault を作成します。ローカル vault が選択したリモート vault とすでに完全に同一である場合、PKV Sync は vault 全体の conflict file を作成する代わりに、一致するファイルをローカル同期インデックスへ取り込みます。

## 同期動作

PKV Sync は Obsidian 内で現在の vault を同期します。

- ローカルファイルの変更は短い debounce 間隔の後に push されます。
- リモート変更は定期的に poll されます。
- 設定ページと command palette から手動同期できます。
- 関連するファイル作成/変更/削除イベントは同期をスケジュールします。
- ウィンドウ blur で同期をトリガーできます。
- 起動時、vault 内容とローカル同期インデックスから未同期のローカル変更を検出します。

大きな添付ファイルをアップロードしている間は Obsidian を開いたままにしてください。接続後、プラグインはサーバー設定を読み取り、サーバーが提供するテキスト拡張子一覧と最大ファイルサイズ規則を使用します。

## 選択的な `.obsidian` 同期

PKV Sync は vault ごとの allowlist を通じて、選択した Obsidian 設定ファイルを同期できます。新しいリモート vault は、テーマ、CSS snippets、ホットキー、アプリ設定、外観設定、有効化済みプラグイン一覧のルールで開始します。

既存のリモート vault は opt-in するまで空の allowlist を保持します。**Settings -> PKV Sync** で現在の vault を選択し、**.obsidian sync rules** を編集して保存します。推奨 starter list ボタンは、新しい vault と同じ starter rules を入力します。

プラグインコードとプラグイン設定は既定では同期されません。`.obsidian/plugins/**` やプラグイン `data.json` ファイルなどの高度なルールを追加する前に、[`dot-obsidian-sync-howto.md`](./dot-obsidian-sync-howto.md) を参照してください。

## 最終同期時刻

設定ページは最後に成功した同期を相対時間で表示します。横の小さな展開コントロールを使うと、次の形式で正確なタイムスタンプを表示できます。

```text
YYYY/MM/DD HH:MM:SS
```

プラグインは選択された IANA タイムゾーンを使用し、既定は `Asia/Shanghai` です。

## 履歴、Diff、復元

サーバーが履歴対応を報告し、プラグイン設定で **Enable history and diff UI** がオンの場合、次の入口からファイル履歴を確認できます。

- **PKV Sync: Show file history**
- ファイル右クリックメニュー: **PKV Sync: File history**
- ファイル右クリックメニュー: **PKV Sync: Diff with previous**

履歴モーダルは現在のファイルの commit を、時刻、装置、commit id、変更種別とともに一覧表示します。テキストファイルでは unified diff を表示できます。バイナリファイルは履歴に表示して復元できますが、PKV Sync はバイナリ diff をレンダリングしません。

バージョンを復元すると、プラグインは選択した履歴内容をサーバーから読み取り、ローカル Obsidian vault に書き戻し、通常の同期エンジンにその書き込みを新しい commit として push させます。現在のローカルファイルが最後に同期した hash と異なる場合、確認ダイアログは未同期のローカル変更が上書きされることを警告します。

PKV Sync はプラグイン内に完全なオフライン履歴キャッシュを保持しません。履歴ビューと diff ビューにはサーバーへの接続が必要です。

## Conflict Files

2 台の装置が同じファイルをオフラインで編集した場合、PKV Sync は両方のバージョンを保持します。リモートまたはローカルの代替バージョンは、生成された conflict file として保存されます。

```text
note.md
note.conflict-2026-04-25-143022-Desktop.md
```

生成された conflict files は以後の同期から除外されます。Obsidian で両方のファイルを確認し、残したい内容を手動でマージしてから conflict file を削除してください。

生成された conflict files は次から管理できます。

- **Settings -> PKV Sync -> Conflict files**
- **PKV Sync: List conflict files**
- **PKV Sync: Delete conflict files**

削除操作は PKV Sync が生成した conflict filename のみを対象にします。`my.conflict-resolution-notes.md` のような通常ファイルは引き続き同期対象です。

## 装置 Token

ログインすると bearer device token が発行されます。認証済みの使用で token は更新されるため、アクティブな装置はサインイン状態を保ち、90 日間アイドルの装置は期限切れになります。プラグインは安定した device ID を保持するため、同じ装置から再ログインすると重複を蓄積せず、その装置の以前のアクティブ token を置き換えます。

Obsidian プラグインはアクティブ token と deployment key を `<vault>/.obsidian/plugins/pkv-sync/data.json` に保存します。このファイルは機密として扱い、平文バックアップやクラウド同期先を保護し、共有しないでください。漏えいした可能性がある場合はログアウトするか、管理者に装置 token の取り消しを依頼してから再接続してください。

- プラグイン設定から現在の装置をログアウトできます。
- 紛失した装置の token は管理者に取り消してもらってください。
- パスワード変更は現在の装置のサインインを維持し、他の装置 token を取り消します。

## MCP アクセス

管理者が `pkvsyncd mcp` コマンドを有効にしている場合、AI ツールは bearer device token を使って MCP 経由で vault にアクセスできます。MCP は vault 一覧、ファイル一覧、HEAD または commit のファイル読み取り、簡単なテキスト検索、そして optimistic concurrency 付きの明示的な write / delete tools を提供します。stdio と Streamable HTTP の設定例は [`mcp-howto.md`](./mcp-howto.md) を参照してください。

## コマンド

PKV Sync は次の command palette actions を追加します。

- Show sync status
- Refresh account info
- Manual sync now
- View sync status details
- List conflict files
- Delete conflict files

## プライバシーの注意

PKV Sync はエンドツーエンド暗号化されていません。サーバー管理者、およびサーバーファイルシステムへアクセスできる人は、同期済み vault 内容と添付ファイルを読めます。信頼できるサーバーと管理者でのみ使用してください。
