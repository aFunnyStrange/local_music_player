> 历史预览版验收记录。后续切换见 [切换记录](cutover-2026-11-21.md)。 Historical pre-cutover acceptance.

# 当前 Windows 环境验收 / Local Windows acceptance

日期：2026-11-21。Python 默认入口和桌面快捷方式未切换，未发布 Release。
Date: 2026-11-21. Python remains the default; shortcut unchanged; no Release published.

## 场景与证据 / Scenarios and evidence

| 场景 / Scenario | 实际结果 / Observed result |
| --- | --- |
| 持续播放 / Endurance | 600 秒实时时钟，前 200 秒同一长 WAV 连续推进，之后列表循环、单曲循环、随机播放；124 次自然续播，未触发 8 秒无进度检测；退出 315 ms / 600 wall-clock seconds, first 200 on one long WAV, then repeat all/one and shuffle; 124 natural restarts, no eight-second stall; shutdown 315 ms. |
| 连续切歌 / Rapid switching | 有界命令队列提交 120 次下一首；计数和最终队列位置精确一致，没有丢失或乱序 / 120 next commands through bounded queue; exact start count and final queue index. |
| 暂停继续 / Pause and resume | 20 组状态转换完成，随后持续播放仍能推进 / 20 pairs completed; subsequent playback progressed. |
| 扫描中退出 / Exit during scan | 1024 首临时音频，实际解析第 17 首后关闭；扫描报告取消，工作任务回收，最终配置完整；重复运行退出约 25–32 ms / Closed after 17 of 1024 fixtures were parsed; cancellation, task completion and final save confirmed; repeated shutdowns about 25–32 ms. |
| 重启恢复 / Restart recovery | 两轮各连续启动三次；每次重新扫描 1024 首，恢复双歌单成员、收藏、名称、选中歌单、目录、语言、音量、模式及关闭策略，并可播放 / Two runs of three restarts each; all 1024 files rescanned, all persisted fields restored, then playback succeeded. |
| 配置导入 / Import | 源码版与已安装 Python 配置各有 3 个目录，全部字段往返一致；源文件字节不变，Rust 旧配置备份有效 / Both Python states contained three roots; all fields round-tripped, source bytes unchanged, prior Rust backup valid. |
| 小窗口 / Small window | 当前显示缩放下 800×540 客户区，中英工具栏、列表、播放/进度/音量控件可见 / Chinese and English toolbars, list and transport/seek/volume controls visible at 800×540 on this display. |
| 保存失败与队列 / Save failure and queues | UI 通知队列满后关闭仍能完成最终保存；非法最终配置明确失败，原配置不损坏 / Closing a full UI notification queue still flushes final state; invalid final state fails without corrupting prior settings. |

静音 WAV 用于可重复实跑；真实 MP3/FLAC/M4A 已在之前的 945 首曲库只读扫描及每格式抽样播放中验证，本轮不将 WAV 耐久结果冒充为全格式长时认证。
Silent WAVs make endurance reproducible. Previous read-only scans covered 945 real tracks with MP3/FLAC/M4A samples; WAV endurance is not certification of every format over long sessions.

后半程资源抽样工作集约 55–57 MiB，句柄约 372–388，未见持续增长。此有限观察不证明长期不存在泄漏。报告及资源原始样本仅保留在被忽略的 `.runtime`，不包含在发行包中。
Late-run working-set samples were about 55–57 MiB and 372–388 handles, with no sustained growth observed. This finite observation does not prove absence of long-term leaks. Raw reports/resources remain in ignored `.runtime`, outside release bundles.

## 本轮修正 / Changes

- 扫描显示已解析数量，能够观测取消是否发生在实际工作中。
  Display parsed counts and observe cancellation during actual work.
- 退出时保留并报告播放/扫描任务异常，同时先完成最终配置与日志写入。
  Report playback/scanner task failures after final state and log flushing.
- 大曲库生命周期自检使用 120 秒时限。原 20 秒时限在正常扫描到 644 首时过早结束，不是解析失败；延长后完整扫描及重启通过。
  Lifecycle diagnostics allow 120 seconds: the original 20-second watchdog stopped a healthy scan at 644 tracks; the full scan and restart passed with the appropriate deadline.
- 新增真实配置临时导入、持续播放、扫描退出和重启的可复跑诊断入口，保持一个异步调度线程、最多两个阻塞池工作线程，无播放器工作子进程。
  Added repeatable diagnostics while retaining one async scheduler, at most two blocking workers and no player worker subprocesses.

## 复跑 / Reproduce

在已登录 Windows 桌面会话中，从 `rust-player` 执行：
From `rust-player` in a logged-in Windows desktop:

```powershell
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/build_release.ps1
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/verify_lifecycle.ps1 -Executable dist/ShiyinRust-0.1.0-dev/ShiyinRust.exe
```

脚本默认 600 秒，`-SoakSeconds 1800` 可延长；`-SkipSoak` 单独复跑扫描退出与三次恢复。所有测试使用独立配置与临时音频。重启恢复不包含自动续播或上次播放位置。
The default is 600 seconds; use `-SoakSeconds 1800` for longer runs or `-SkipSoak` for lifecycle-only checks. Tests isolate settings/audio. Restart recovery does not mean automatic playback or resuming the previous position.

干净 Windows 机器、其他声卡/设备切换、跨屏 DPI 和数小时运行仍未验收，保持迁移清单未勾选状态。
Clean Windows machines, other audio devices/device switching, cross-monitor DPI and multi-hour operation remain unverified.
