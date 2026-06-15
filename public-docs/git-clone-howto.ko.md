# PKV vault를 Git clone하기

[English](./git-clone-howto.md) | [简体中文](./git-clone-howto.zh-CN.md) | [繁體中文](./git-clone-howto.zh-Hant.md) | [日本語](./git-clone-howto.ja.md) | 한국어

문서 버전: v1.4.3.

PKV Sync는 각 vault를 HTTPS를 통한 read-only Git repository로 노출할 수 있습니다.

## Prerequisites

- Server admin이 Sync & Storage settings에서 “Git smart HTTP”를 활성화했습니다.
- Server에서 `git` binary를 사용할 수 있습니다.
- 유효한 device token이 있습니다.

## Clone

```bash
git clone https://_:<token>@your-server/git/<vault-id>
```

콜론 앞의 underscore는 username입니다. 어떤 값이어도 됩니다. password 위치의 token만 사용됩니다.

### Example

server가 `sync.example.com`, vault ID가 `6c0a2b8f4d3e419a8c5b7f1d2e3a4b5c`, device token이 `pks_0f1e2d3c4b5a6978...`라면 다음을 실행합니다.

```bash
git clone https://_:pks_0f1e2d3c4b5a6978@sync.example.com/git/6c0a2b8f4d3e419a8c5b7f1d2e3a4b5c
```

Vault ID는 32자 소문자 hex입니다(대시 없음). Admin WebUI와 `pkvsyncd user list`가 유효한 ID를 보여줍니다. `abc123` 같은 placeholder는 `400 invalid_vault_id`로 거부됩니다.

## Materialize

clone 후에는 PKV Sync server가 큰 파일을 별도로 저장하기 때문에 blob files가 pointer JSON으로 보입니다. 다음을 실행합니다.

```bash
pkvsyncd materialize <vault-id> -o ./output
```

pointer files를 실제 binary content로 바꾸어 완전히 사용할 수 있는 로컬 vault copy를 만듭니다.

## Notes

- HTTP를 통한 repository는 **read-only**입니다. Git으로 변경사항을 push할 수 없습니다.
- 변경은 PKV Sync plugin에서 수행하고 일반 sync API로 push하세요.
- Server admin이 Git smart HTTP를 비활성화하면 clone 또는 fetch가 HTTP 503을 반환합니다.
