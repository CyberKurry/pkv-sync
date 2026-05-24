# 安全政策

[English](./SECURITY.md) | [简体中文](./SECURITY.zh-CN.md) | 繁體中文 | [日本語](./SECURITY.ja.md) | [한국어](./SECURITY.ko.md)

## 支援版本

PKV Sync 從 v1.0.0 起遵循語義化版本。安全修復維護目前 minor 線和上一條 minor 線。

| 版本 | 狀態 | 安全支援結束時間 |
| --- | --- | --- |
| 最新 1.x minor | 活躍支援 | 待定 |
| 上一個 1.x minor | 僅安全修復 | 下一條 1.x minor 發布時 |
| 0.x | 不支援 | v1.0.0 發布時 |

## 報告漏洞

請不要為安全漏洞建立公開 GitHub issue。

首選通道：透過 `cyberkurry/pkv-sync` 的 GitHub Security Advisories 提交私密報告。

請包含：

- 受影響的 PKV Sync 版本。
- 最小復現步驟。
- 影響評估。
- 如果你已經有建議修復方案，也請一併提供。

## 回應目標

- 初次確認：5 個工作日內。
- 嚴重性分級：10 個工作日內。
- 修復與協調披露：critical/high 級別 90 天內，medium/low 級別 180 天內。
- CVE：適用時透過 GitHub Security Advisories 分配。

## 範圍

範圍內：

- `pkvsyncd` 服務端二進位。
- Obsidian 外掛。
- Admin Web UI。
- MCP stdio 和 Streamable HTTP transport。
- 推薦了不安全部署方式的公開文件。

範圍外：

- PKV Sync 之外的主機、反向代理、TLS、Docker、systemd 和作業系統加固。
- 第三方 Obsidian 外掛。
- 在 PKV Sync 使用方式下不可利用的依賴漏洞。
- 需要管理員權限或已經攻陷主機之後才能成立的報告。

## 已知非問題

- PKV Sync 1.0 預設把普通筆記庫內容以明文形式存放在服務端，這是設計選擇。原生 per-vault E2EE 計畫進入 1.x 路線圖。今天就需要客戶端側加密的使用者可以使用 [`git-crypt`](./public-docs/git-crypt-howto.zh-Hant.md)，並接受 README 中說明的取捨。
- `/metrics` 預設關閉。啟用後，在生產 server stack 中仍然需要部署金鑰中介軟體、被接受的 PKV Sync User-Agent 和管理員 bearer token。
- MCP HTTP 同時需要部署金鑰和 bearer token 認證；是否公開暴露由維運者決定，但應像其他認證後的管理相鄰入口一樣保護。

## 披露

PKV Sync 遵循協調披露。除非報告者希望匿名，否則會在安全公告和 `CHANGELOG.md` 中致謝。
