> **Ghi chú khi đưa vào repo (2026-11-22).** File này là bản export của design doc trên claude.ai (artifact `K47EPMgWrZgCv37Z7zSzyH`), giữ nguyên nội dung. Đây là nguồn chuẩn cho **hành vi**; `DESIGN_SYSTEM.md` là nguồn chuẩn cho **giao diện** (HANDOFF.md §1). Các quyết định sau của owner mới hơn và thay thế phần tương ứng trong doc:
>
> - Đánh số bước: doc gọi spike là bước 1; HANDOFF.md gọi là bước 0. Spike đã xong và merge (PR #63, `apps/pod-shape/spike/`). Bước "Khung app" ở đây là bước 1 của HANDOFF.md.
> - Câu hỏi mở 1: chỉ macOS cho V1 — đồng ý.
> - Câu hỏi mở 3: ADR trong app — đồng ý, git chỉ trong `docs/adr/` + `gh pr create`.
> - Câu hỏi mở 4: commit prototype HTML lên GitHub — **không**.
> - Câu hỏi mở 5: bỏ trạng thái "running" khỏi V1 (pd-bot không có tín hiệu rõ).
> - Câu hỏi mở 6: code nằm trong repo pb-bot, thư mục `apps/pod-shape` (không phải repo riêng).
> - Bộ flag `claude -p` ở mục Kiến trúc đã được thay bằng bộ flag đã kiểm trong `apps/pod-shape/spike/README.md` (`--permission-mode dontAsk`, allowlist theo từng bước).
> - Mục "Tạo milestone và issues": app chặn mọi lệnh ghi GitHub cho tới khi owner bấm duyệt (xem spike README).

# Shape Board — Thiết kế app V1

Nov 22, 2026

## Tóm tắt

Shape Board là app desktop cá nhân, chạy trên máy anh. Nó đưa một prototype (Claude artifact) qua bước grill, rồi tạo GitHub milestone và issues. App còn cho một view tổng quan: repo nào có artifact nào, artifact ra feature nào, feature ra milestone và issue nào, issue đang ở trạng thái gì.

**V1 làm:**

- Thêm project = 1 GitHub repo (kèm thư mục clone trên máy).
- Dán artifact prototype: link + nội dung HTML. App lưu một bản copy.
- Grill/brainstorm ngay trong app. App chạy Claude Code nền bằng subscription của anh, dùng skill `pd-shape`. Kết quả là 1 brief cho 1 feature.
- Bấm nút để tạo milestone + issues từ brief, theo cách của `pd-tickets` (anh duyệt 1 lần).
- View cây: Project → Artifact → Feature → Milestone → Issue, trạng thái issue đọc trực tiếp từ GitHub.
- Lưu đầy đủ lịch sử grill trong SQLite.

**V1 không làm:**

- Không tự lấy danh sách project/artifact từ claude.ai (không có API chính thức, và điều khoản không cho app đọc cookie/session của claude.ai).
- Không có nút chạy podbay / autopilot.
- Không có chế độ auto; mọi bước do anh bấm.
- Không đồng bộ nhiều máy; dữ liệu chỉ ở máy này.
- Logo và design system làm ở thread khác; tài liệu này chỉ nói chức năng.

## Luồng người dùng

Một vòng đầy đủ có 5 bước. Mỗi bước có "điểm xong" rõ ràng.

1. **Thêm project.** Anh chọn repo (`owner/name`) và thư mục clone trên máy. App kiểm tra `gh` đã đăng nhập và thư mục đúng là repo đó. Xong khi project hiện trong danh sách.
2. **Dán artifact.** Anh dán link artifact và nội dung HTML, đặt tên ngắn (slug). App lưu bản copy và hiện preview. Dán lại lần sau = một version mới của cùng artifact. Xong khi preview hiện đúng.
3. **Grill.** Anh bấm "Grill" trên artifact. App mở một phiên Claude Code nền trong thư mục repo, chạy `pd-shape` với file prototype. Câu hỏi hiện thành thẻ có lựa chọn (giống AskUserQuestion); anh bấm hoặc gõ trả lời. Anh có thể dừng và quay lại sau. Xong khi anh trả lời "đúng ý" cho brief. 1 artifact có thể grill nhiều lần, mỗi lần ra 1 feature.
4. **Tạo issues.** Trên feature đã có brief, anh bấm "Generate issues". App cho xem trước: milestone (nếu có từ 2 issue), danh sách issue, nội dung từng issue. Anh sửa hoặc bấm "Create" một lần. Xong khi mọi issue có số trên GitHub.
5. **Xem tổng quan.** View cây cho mọi project. Trạng thái issue tải lại từ GitHub khi mở view hoặc bấm Refresh.

## Kiến trúc

App là Tauri 2: backend Rust, giao diện web (đề xuất React + TypeScript + Vite). App không gọi API của Anthropic hay GitHub trực tiếp. Nó chạy 2 CLI có sẵn trên máy anh: `claude` (đăng nhập bằng subscription) và `gh` (đăng nhập GitHub). Nhờ vậy app không giữ API key hay token nào.

&#91;embedded content: kiến trúc · app, 2 CLI, GitHub\]

Backend Rust là nơi duy nhất chạy lệnh; giao diện chỉ hiển thị và gửi lệnh qua Tauri.

**Các module Rust:**

| Module | Việc | Công cụ |
| --- | --- | --- |
| `db` | Đọc/ghi SQLite, migration | `rusqlite` hoặc `sqlx` |
| `claude_runner` | Chạy `claude -p`, đọc output từng dòng JSON, báo UI qua Tauri event, hủy khi anh bấm Stop | `tokio::process` |
| `grill` | Quản lý phiên grill: tạo, resume, tách câu hỏi, lưu tin nhắn, đọc brief | dùng `claude_runner` |
| `tickets` | Lập kế hoạch issue từ brief, tạo milestone/issue | `claude_runner` + `gh` |
| `github` | Đọc trạng thái issue/PR, tạo milestone/issue | `gh api` (GraphQL/REST) |
| `artifacts` | Lưu HTML, ghi file prototype ra thư mục repo khi cần | file system |

**Cách gọi Claude Code (mỗi lượt grill = 1 lần chạy):**

```bash
claude -p "<tin nhắn>" \
  --session-id <uuid>          # lượt đầu; các lượt sau dùng --resume <uuid>
  --output-format stream-json --verbose \
  --append-system-prompt "<giao thức hỏi đáp của app>" \
  --allowedTools "Read,Grep,Glob,Write(specs/**),Bash(gh issue view *)" \
  --disallowedTools "AskUserQuestion"
```

Lệnh chạy với thư mục hiện tại = thư mục clone của repo, nên Claude đọc được code và dùng plugin pd-bot đã cài. Claude Code tự lưu transcript ở `~/.claude/projects/`; app lưu thêm bản của mình vào SQLite để hiển thị và tìm kiếm.

**Về "performance cao":** Rust + Tauri cho app nhẹ (ít RAM, file cài nhỏ) và UI mượt. Nhưng phần chậm thật sự là thời gian Claude trả lời (vài giây đến vài phút) và gọi mạng tới GitHub. Rust không làm phần đó nhanh hơn.

**Có thể dùng lại:** repo pb-bot đã có Pod Board (`apps/pod-board`), cũng là Tauri. Có thể copy cấu hình build của nó để khởi động nhanh hơn.

## Grill

App không viết lại logic grill. Nó gọi `pd-shape` của plugin pd-bot và chỉ thay cách hỏi đáp: thay vì hỏi trong terminal, câu hỏi hiện trong app.

**Vấn đề cần giải:** `pd-shape` hỏi bằng tool `AskUserQuestion`. Khi chạy nền (`claude -p`) không có ai trả lời tool đó. Cách đề xuất:

1. App tắt `AskUserQuestion` và thêm một đoạn system prompt: "Khi cần hỏi owner, hãy dừng lượt và in một khối `p2g-questions` dạng JSON, cùng cấu trúc với input của AskUserQuestion."
2. App đọc khối đó và vẽ thẻ câu hỏi: tiêu đề, 2–4 lựa chọn, lựa chọn "(Recommended)" đứng đầu, phần preview.
3. Anh trả lời. App gửi câu trả lời là lượt tiếp theo với `--resume <session-id>`.
4. Khi brief sẵn sàng, Claude in khối `p2g-brief` kèm đường dẫn `specs/<slug>/brief.md`. App đọc file, lưu vào SQLite, hiện brief và hỏi "Brief này đúng ý anh chưa?".

Cách này không cần sửa `pd-shape`. Nếu Claude không theo giao thức đều đặn, phương án dự phòng là thêm một mục `Mode: app` nhỏ vào `pd-shape` (một PR vào pb-bot). Hai điểm này phải thử thật trước khi code (bước Spike).

**Lượt đầu tiên** gửi: `/pd-bot:pd-shape <đường dẫn file prototype>` + ghi chú của anh (nếu có). File prototype là bản HTML app đã lưu, ghi ra đĩa để Claude đọc.

**Trạng thái một phiên grill:**

| Trạng thái | Nghĩa |
| --- | --- |
| `running` | Claude đang trả lời; anh có thể bấm Stop |
| `waiting_owner` | Có câu hỏi chờ anh |
| `brief_ready` | Có brief, chờ anh xác nhận |
| `confirmed` | Anh nói "đúng ý"; feature được tạo |
| `stopped` | Anh dừng; mở lại được bằng `--resume` |
| `failed` | Lỗi (ví dụ hết hạn mức subscription); hiện lỗi gốc |

**Lịch sử:** mỗi tin nhắn (của anh, của Claude, câu hỏi, câu trả lời, tool call) lưu 1 dòng trong SQLite. Đúng như anh nói: mỗi grill là một phiên Claude Code chạy nền, và app giữ `session_id` để tiếp tục.

**ADR:** `pd-shape` có thể hỏi "Make this an ADR?". Nếu anh đồng ý, nó cần quyền `git commit`, `git push` và `gh pr create`. Xem câu hỏi mở số 3.

## Tạo milestone và issues

Đề xuất: app chạy chính `pd-tickets`, không tự viết luật tạo issue. Lý do: luật đã có sẵn (issue phải qua `pd-intake` là READY, có dòng `Verify:` thật, có dòng `source:` để chạy lại không bị trùng). Viết lại trong Rust sẽ có hai bản luật và dễ lệch nhau.

**Các bước khi anh bấm "Generate issues":**

1. App chạy `claude -p "/pd-bot:pd-tickets specs/<slug>/brief.md"` trong thư mục repo, cùng giao thức hỏi đáp như grill.
2. Bước "owner duyệt 1 lần" của `pd-tickets` hiện thành màn preview: milestone, danh sách issue, nội dung từng issue. Anh bấm Create hoặc gõ yêu cầu sửa.
3. `pd-tickets` tạo issue/milestone bằng `gh` (đã cho phép qua `--allowedTools`).
4. App tìm lại issue vừa tạo bằng dòng `source:` (qua `gh`), ghi số issue, milestone và feature vào SQLite. Bước này không dựa vào câu chữ Claude in ra, nên ít sai.

**Luật đã thống nhất ở các thread trước vẫn giữ:** feature mới từ 2 issue trở lên → có milestone; cập nhật nhỏ → 1 issue. Nội dung brief đi vào từng issue, brief không commit. ADR chỉ cho quyết định cấp repo, đi vào main qua PR.

**Chạy lại:** nếu brief đổi và anh bấm Generate lần nữa, `pd-tickets` dùng dòng `source:` để cập nhật thay vì tạo trùng. App hiện rõ issue nào mới, issue nào sửa.

## Data model (SQLite)

Một file SQLite trong thư mục dữ liệu của app (ví dụ `~/Library/Application Support/Shape Board/shape-board.db` trên macOS). Các bảng chính:

| Bảng | Giữ gì | Liên kết |
| --- | --- | --- |
| `projects` | repo `owner/name`, thư mục clone, ngày thêm | — |
| `artifacts` | tên, slug, link claude.ai | `project_id` |
| `artifact_versions` | HTML đã dán, hash, ngày dán | `artifact_id` |
| `grill_sessions` | `claude_session_id`, trạng thái, bắt đầu/kết thúc | `artifact_version_id` |
| `grill_messages` | vai (owner/claude/tool), nội dung, JSON gốc, thời gian | `grill_session_id` |
| `features` | tên feature, trạng thái | `artifact_id`, `grill_session_id` |
| `briefs` | nội dung brief.md, số version, đã xác nhận chưa | `feature_id` |
| `milestones` | số và tên milestone trên GitHub | `feature_id` |
| `issues` | số issue, tiêu đề, URL; trạng thái đọc lần cuối + giờ đọc | `feature_id`, `milestone_id` |
| `runs` | mỗi lần chạy `claude`: lệnh, exit code, thời gian, lỗi | `grill_session_id` hoặc `feature_id` |

Trạng thái issue lưu trong SQLite chỉ là cache để mở app nhanh. Nguồn đúng luôn là GitHub.

> **Owner-approved (advisor change), 2026-11-23, build step 2** (`src-tauri/migrations/0002_artifacts.sql`):
>
> - `artifacts.project_id` may be NULL: an artifact the hook saved that is in no project yet is in the "New artifacts" box (HANDOFF §11.4.2).
> - `artifacts.claude_id` (the last part of the claude.ai link) may be NULL: an artifact added by dropping a file has no link until one is pasted.
> - `artifacts.state` is `new` (in the box), `added` (in a project) or `ignored`, with `CHECK ((state = 'added') = (project_id IS NOT NULL))`. Also `source` (`hook` or `manual`), `updated` (a new version arrived; cleared when the owner opens the preview), `title`, `cwd` and `published_at` from the log.
> - `artifact_versions` stores `n`, `html` (at most 5 MB), `sha256`, `source` (`hook`, `drop`, `paste`) and the hook `file` (unique when set). Two more tables, `hook_files` and `hook_state`, remember which hook files were read and the last log line read.

## Màn hình

V1 có 4 màn hình. Giao diện theo design system của thread design; tài liệu này chỉ nói nội dung.

| Màn hình | Nội dung | Hành động |
| --- | --- | --- |
| Home | Danh sách project. Mỗi project: số artifact, số grill đang chờ anh, số issue open / có PR / merged | Thêm project, mở project |
| Project (view cây) | Cây Artifact → Feature → Milestone → Issue, bên phải là chi tiết mục đang chọn | Dán artifact, Grill, Generate issues, Refresh |
| Grill | Preview prototype bên trái, hội thoại + thẻ câu hỏi bên phải, brief hiện khi sẵn sàng | Trả lời, Stop, Resume, Xác nhận brief |
| Issue preview | Milestone + danh sách issue trước khi tạo | Create, Yêu cầu sửa |

**Trạng thái issue** (đọc bằng `gh api graphql`, một lần gọi cho cả project):

| Trạng thái | Cách nhận biết |
| --- | --- |
| open | Issue mở, chưa có PR liên kết |
| running | Issue mở và có dấu hiệu pd-bot đang làm (chưa rõ là label nào: câu hỏi mở số 5) |
| has PR | Có PR mở liên kết với issue |
| merged | Issue đóng bởi một PR đã merge |
| closed | Issue đóng mà không có PR merge |

Mỗi feature và milestone hiện thanh tiến độ: bao nhiêu issue đã merged trên tổng số.

**Preview prototype an toàn:** HTML dán vào có thể chứa script. App hiển thị trong iframe `sandbox="allow-scripts"` (không có `allow-same-origin`), nên script của prototype chạy được nhưng không gọi được lệnh Tauri, không đọc được file hay dữ liệu app.

## Rủi ro và giới hạn

Rủi ro lớn nhất là kỹ thuật: `pd-shape` có chạy ổn trong `claude -p` với giao thức hỏi đáp của app không. Vì vậy bước đầu tiên là Spike, không phải code UI.

| Rủi ro | Mức | Cách xử lý |
| --- | --- | --- |
| Claude không theo giao thức `p2g-questions` đều đặn | Cao | Spike trước. Dự phòng: thêm mục `Mode: app` vào `pd-shape` |
| Slash command `/pd-bot:...` trong `claude -p` không chạy như mong đợi | Trung bình | Spike. Dự phòng: gửi nội dung skill vào prompt |
| Điều khoản: subscription cho phép "ordinary, individual usage" của Claude Code. Script cá nhân gọi `claude -p` có lẽ phù hợp, nhưng tài liệu không nói rõ ([legal and compliance](https://code.claude.com/docs/en/legal-and-compliance)) | Trung bình | Chỉ dùng cá nhân, không phân phối app cho người khác, không chạy tự động hàng loạt. Nếu sau này muốn chia sẻ app, cần hỏi lại Anthropic hoặc chuyển sang API key |
| Hết hạn mức subscription giữa phiên grill | Trung bình | Hiện lỗi gốc, phiên về `failed`, resume sau được |
| `pd-shape` chưa merge (PR #59) và sẽ còn đổi | Trung bình | App kiểm tra plugin pd-bot có `pd-shape` khi khởi động; giao thức chỉ nằm trong system prompt của app |
| HTML prototype chứa script lạ | Thấp | iframe sandbox, không có quyền Tauri |
| Claude ghi file hoặc chạy lệnh ngoài ý | Thấp | `--allowedTools` chỉ mở đúng lệnh cần; không dùng `--dangerously-skip-permissions` |
| Dán tay link + HTML tốn công | Thấp | Chấp nhận cho V1; không có cách chính thức để tự lấy |

## Câu hỏi mở cần anh quyết

Mỗi câu có đề xuất của tôi. Anh chỉ cần trả lời "ok" hoặc chọn khác.

1. **Chỉ macOS cho V1?** Đề xuất: có, vì máy Mac của anh là nơi chạy Pod Bay. Windows/Linux để sau.
2. **Frontend: React + TypeScript hay Svelte?** Đề xuất: React + TypeScript. Phổ biến nhất, Claude Code viết tốt, dễ theo design system.
3. **ADR trong app:** khi `pd-shape` hỏi "Make this an ADR?" và anh đồng ý, app có cho Claude commit, push và mở PR không? Đề xuất: có, nhưng chỉ mở quyền `git` cho `docs/adr/` và `gh pr create`. Phương án khác: V1 bỏ qua ADR, anh làm ADR trong terminal.
4. **File prototype trên GitHub:** thread trước đã thống nhất commit HTML vào `prototypes/<slug>.html` trên một branch và issue link tới đó. App có tự làm việc này khi bấm Generate không? Đề xuất: có, branch `prototype/<slug>`, app hiện lệnh trước khi push.
5. **Trạng thái "running":** pd-bot đánh dấu issue đang chạy bằng gì (label, comment, branch)? Tôi sẽ tự đọc repo pb-bot để tìm; nếu không có dấu hiệu rõ, V1 bỏ trạng thái này và chỉ có open / has PR / merged / closed.
6. **Repo cho code của app:** đề xuất tạo repo mới `shape-board` (private) trên GitHub của anh. Tôi không tạo được repo; anh tạo rồi báo tôi.

## Kế hoạch build

Mỗi bước là 1 PR nhỏ, có điểm xong rõ ràng. Không bắt đầu khi anh chưa duyệt tài liệu này.

1. **Spike (làm trước, không UI).** Script nhỏ chạy `pd-shape` qua `claude -p` + `--resume` trên 1 prototype thật. Xong khi: 3 lần chạy liền đều ra khối `p2g-questions` đúng JSON và kết thúc bằng `brief.md`. Nếu thất bại, quay lại anh trước khi làm tiếp.
2. **Khung app.** Tauri 2 + React, SQLite + migration, màn Home, thêm project (kiểm tra `gh`, `claude`, thư mục repo). Xong khi thêm được 1 repo và mở lại app vẫn thấy.
3. **Artifact.** Dán link + HTML, lưu version, preview trong iframe sandbox. Xong khi prototype chạy được trong app.
4. **Grill.** `claude_runner` + màn Grill + thẻ câu hỏi + Stop/Resume + lưu lịch sử + xác nhận brief. Xong khi grill trọn vẹn 1 prototype thật, đóng app giữa chừng rồi resume được.
5. **Generate issues.** Chạy `pd-tickets`, màn preview, tạo issue, ghi liên kết vào SQLite. Xong khi issue thật xuất hiện trên GitHub và trong cây.
6. **View cây + trạng thái.** Cây đầy đủ, đọc trạng thái bằng 1 lần gọi GraphQL, thanh tiến độ. Xong khi 2 project với issue ở các trạng thái khác nhau hiện đúng.

Bước 1 phải chạy trên máy anh (cần `claude` đăng nhập subscription và plugin pd-bot có `pd-shape`).
