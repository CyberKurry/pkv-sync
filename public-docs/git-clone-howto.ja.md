# PKV vault 銈?Git clone 銇欍倠

[English](./git-clone-howto.md) | [绠€浣撲腑鏂嘳(./git-clone-howto.zh-CN.md) | [绻侀珨涓枃](./git-clone-howto.zh-Hant.md) | 鏃ユ湰瑾?| [頃滉淡鞏碷(./git-clone-howto.ko.md)

銉夈偔銉ャ儭銉炽儓銉愩兗銈搞儳銉? v1.4.5銆?

PKV Sync 銇€佸悇 vault 銈?HTTPS 绲岀敱銇?read-only Git repository 銇ㄣ仐銇﹀叕闁嬨仹銇嶃伨銇欍€?

## Prerequisites

- Server admin 銇?Sync & Storage settings 銇с€孏it smart HTTP銆嶃倰鏈夊姽鍖栥仐銇︺亜銈嬨€?
- Server 涓娿仹 `git` binary 銇屽埄鐢ㄣ仹銇嶃倠銆?
- 鏈夊姽銇?device token 銈掓寔銇ｃ仸銇勩倠銆?

## Clone

```bash
git clone https://_:<token>@your-server/git/<vault-id>
```

銈炽儹銉冲墠銇?underscore 銇?username 銇с仚銆傚€ゃ伅浣曘仹銈傛銇勩伨銇涖倱銆俻assword 閮ㄥ垎銇?token 銇犮亼銇屼娇銈忋倢銇俱仚銆?

### Example

server 銇?`sync.example.com`銆乿ault ID 銇?`6c0a2b8f4d3e419a8c5b7f1d2e3a4b5c`銆乨evice token 銇?`pks_0f1e2d3c4b5a6978...` 銇牬鍚堬細

```bash
git clone https://_:pks_0f1e2d3c4b5a6978@sync.example.com/git/6c0a2b8f4d3e419a8c5b7f1d2e3a4b5c
```

Vault ID 銇?32 鏂囧瓧銇皬鏂囧瓧 hex锛坉ash 銇仐锛夈仹銇欍€侫dmin WebUI 銇?`pkvsyncd user list` 銇ф湁鍔广仾 ID 銈掔⒑瑾嶃仹銇嶃伨銇欍€俙abc123` 銇倛銇嗐仾 placeholder 銇?`400 invalid_vault_id` 銇ф嫆鍚︺仌銈屻伨銇欍€?

## Materialize

clone 寰屻€丳KV Sync server 銇屽ぇ銇嶃仾銉曘偂銈ゃ儷銈掑垾閫斾繚瀛樸仐銇︺亜銈嬨仧銈併€乥lob files 銇?pointer JSON 銇ㄣ仐銇﹁〃绀恒仌銈屻伨銇欍€傛銈掑疅琛屻仐銇俱仚銆?

```bash
pkvsyncd materialize <vault-id> -o ./output
```

pointer files 銈掑疅闅涖伄 binary content 銇疆銇嶆彌銇堛€佸畬鍏ㄣ伀鍒╃敤鍙兘銇儹銉笺偒銉?vault copy 銈掔敓鎴愩仐銇俱仚銆?

## Notes

- HTTP 绲岀敱銇?repository 銇?**read-only** 銇с仚銆侴it 銇у鏇淬倰 push 銇с亶銇俱仜銈撱€?
- 澶夋洿銇?PKV Sync plugin 銇ц銇勩€侀€氬父銇?sync API 銇?push 銇椼仸銇忋仩銇曘亜銆?
- Server admin 銇?Git smart HTTP 銈掔劇鍔瑰寲銇欍倠銇ㄣ€乧lone 銈?fetch 銇?HTTP 503 銈掕繑銇椼伨銇欍€?
