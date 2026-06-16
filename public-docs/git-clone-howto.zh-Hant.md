# 閫忛亷 Git clone 浣犵殑 PKV vault

[English](./git-clone-howto.md) | [绠€浣撲腑鏂嘳(./git-clone-howto.zh-CN.md) | 绻侀珨涓枃 | [鏃ユ湰瑾瀅(./git-clone-howto.ja.md) | [頃滉淡鞏碷(./git-clone-howto.ko.md)

鏂囦欢鐗堟湰锛歷1.4.5銆?

PKV Sync 鍙互灏囨瘡鍊?vault 閫忛亷 HTTPS 鏆撮湶鐐哄敮璁€ Git repository銆?

## 鍓嶇疆姊濅欢

- Server admin 宸插湪 Sync & Storage settings 鍟熺敤銆孏it smart HTTP銆嶃€?
- Server 涓婃湁鍙敤鐨?`git` binary銆?
- 浣犳搧鏈夋湁鏁堢殑 device token銆?

## Clone

```bash
git clone https://_:<token>@your-server/git/<vault-id>
```

鍐掕櫉鍓嶇殑搴曠窔鏄?username銆傚彲濉换鎰忓€硷紱鍙湁 password 浣嶇疆鐨?token 鏈冭浣跨敤銆?

### 绡勪緥

濡傛灉 server 鏄?`sync.example.com`銆乿ault ID 鏄?`6c0a2b8f4d3e419a8c5b7f1d2e3a4b5c`銆乨evice token 鏄?`pks_0f1e2d3c4b5a6978...`锛屽煼琛岋細

```bash
git clone https://_:pks_0f1e2d3c4b5a6978@sync.example.com/git/6c0a2b8f4d3e419a8c5b7f1d2e3a4b5c
```

Vault ID 鏄?32 鍊嬪瓧鍏冪殑灏忓 hex锛堜笉鍚€ｅ瓧铏燂級銆侫dmin WebUI 鑸?`pkvsyncd user list` 鏈冮’绀烘湁鏁?ID锛涘儚 `abc123` 閫欓浣斾綅瀛椾覆鏈冭浠?`400 invalid_vault_id` 鎷掔禃銆?

## Materialize

Clone 涔嬪緦锛宐lob 妾旀鏈冮’绀虹偤 pointer JSON锛屽洜鐐?PKV Sync server 鏈冨柈鐛ㄥ劜瀛樺ぇ妾旀銆傚煼琛岋細

```bash
pkvsyncd materialize <vault-id> -o ./output
```

閫欐渻灏?pointer 妾旀鏇挎彌鐐哄闅涗簩閫蹭綅鍏у锛岀敘鐢熷畬鏁村彲鐢ㄧ殑鏈 vault copy銆?

## 娉ㄦ剰浜嬮爡

- HTTP 涓婄殑 repository 鏄?*鍞畝**銆備綘涓嶈兘閫忛亷 Git push 璁婃洿銆?
- 璜嬩娇鐢?PKV Sync 澶栨帥閫茶璁婃洿锛屼甫閫忛亷涓€鑸?sync API push銆?
- 濡傛灉 server admin 鍋滅敤 Git smart HTTP锛宑lone 鎴?fetch 鏈冨洖鍌?HTTP 503銆?
