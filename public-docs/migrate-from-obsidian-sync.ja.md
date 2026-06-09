# Obsidian Sync から移行する

[English](./migrate-from-obsidian-sync.md) | [简体中文](./migrate-from-obsidian-sync.zh-CN.md) | [繁體中文](./migrate-from-obsidian-sync.zh-Hant.md) | 日本語 | [한국어](./migrate-from-obsidian-sync.ko.md)

ドキュメントバージョン: v1.1.1。

この文書は機械翻訳による初版です。公開前にネイティブ話者によるレビューを推奨します。

このガイドでは、すでに Obsidian Sync を使っている Obsidian vault の現在のファイルを、新しい PKV Sync vault に取り込む方法を説明します。

移行では、この装置に現在存在するファイルだけを取り込みます。Obsidian Sync の履歴、リモートのバージョン履歴、削除済みファイルの履歴、競合メタデータは取り込みません。PKV Sync の履歴は、新しい PKV vault を作成する移行 commit から始まります。

移行は Obsidian Sync を無効化、アンインストール、変更しません。PKV Sync の結果を確認したあとで Obsidian Sync を停止したい場合は、Obsidian で手動でオフにしてください。

## 始める前に

- 移行に使う装置で Obsidian Sync の同期が完了するまで待ちます。
- 移行前に vault フォルダーを手動でバックアップします。
- 可能であれば、取り込み中は Obsidian を閉じるか、少なくともファイル編集を避けます。
- 移行先の PKV Sync サーバーアカウントを先に作成または確認します。

## 取り込まれるもの

PKV Sync は新しい vault を作成し、現在の取り込み内容を最初の PKV 履歴 entry として commit します。

通常の Markdown ファイル、添付ファイル、一般的な vault ファイルは、PKV Sync の強制除外に一致しない限り取り込まれます。

## スキップされるもの

インポーターは Obsidian Sync の内部ファイル、PKV Sync plugin 自身の状態、OS のジャンクファイル、ローカル実行時ファイルをスキップします。例：

- `.obsidian/sync/`
- `.obsidian/workspace.json`
- `.obsidian/workspace-mobile.json`
- `.obsidian/workspaces.json`
- `.obsidian/cache/**`
- `.obsidian/plugins/pkv-sync/`（plugin 自身の設定と token store はローカル限定で保持されます）
- `.trash/**`
- `.git/**`
- `.DS_Store`（macOS）
- `Thumbs.db`（Windows）
- `*.tmp` や `*.lock` などの一時ファイル
- 装置固有の workspace、cache、trash、一時ファイル

選択した `.obsidian` 設定ファイルは、あとで vault ごとの `.obsidian` allowlist から同期できます。詳しい規則は `.obsidian` 設定同期ガイドを参照してください。

## 移行後

別の装置で新しい PKV vault を開き、ノートと添付ファイルが正しく見えることを確認します。確認が終わるまで、手動バックアップは保持してください。

Obsidian Sync と PKV Sync を同じフォルダーで動かし続ける場合は、慎重に変更してください。2 つの同期システムが同じファイルで競合する可能性があり、PKV Sync は移行 commit 以後に受け取った変更だけを記録します。
