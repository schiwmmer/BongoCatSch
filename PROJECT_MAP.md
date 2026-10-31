# 📂 Bingocat — PROJECT MAP

*(自动生成于 2026-11-01 00:44)*

## One-Line Overview

BongoCat 桌宠（Rust + gpui）；本仓库 Python 脚本仅用于本地化校验与发布/打包契约测试。

---

## 数据流

```
cargo 构建 → verify 流水线跑 scripts/tests/*.py 契约测试
  （校验 CHANGELOG、changed-paths 分类、打包产物、i18n 目录）
  → cargo-packager 出包 → 发布
```

---

<!-- BEGIN GENERATED: dep-graph -->
```
```

> 箭头方向：`A → B` 表示 A 的代码 import 了 B 的模块。
<!-- END GENERATED: dep-graph -->

---

<!-- BEGIN GENERATED: file-table -->
## 子包概览

| 子包 | 文件数 | 总行数 |
|------|-------:|-------:|
| `tools/` | 22 | 4426行 |

## 文件列表


### 📦 tools/
| 文件 | 行数/KB | 类型 | 关键函数 | 被引用 | 修改时间 |
|------|--------:|------|---------|-------:|---------:|
| `tools/validate-fixtures.py` | 542行/24KB | tool | fail(), load(), validate_input() | — | 2026-10-14 |
| `tools/tests/test_release_changelog_contract.py` | 482行/19KB | tool | read(), rust_string_constant(), level_two_heading() | — | 2026-10-26 |
| `tools/tests/test_packaging_contract.py` | 480行/23KB | tool | read(), shipped_targets(), test_exactly_two_release_targets_ship() | — | 2026-10-31 |
| `tools/inspect-cubism-sdk.py` | 439行/16KB | entry | sha256_file(), git_blob_sha(), normalized_archive_name() | — | 2026-10-29 |
| `tools/validate-json-schema.py` | 246行/9KB | tool | load(), validate_schema(), validate_directory() | — | 2026-10-29 |
| `tools/run-input-fixtures.py` | 234行/9KB | tool | load(), button_key(), parameter_names() | — | 2026-10-14 |
| `tools/tests/test_update_release_contract.py` | 222行/8KB | tool | read(), section(), rust_string_constant() | — | 2026-10-29 |
| `tools/tests/test_inspect_cubism_sdk.py` | 197行/7KB | tool | valid_entries(), write_zip(), inspect() | — | 2026-10-29 |
| `tools/tests/test_product_version_contract.py` | 187行/6KB | tool | read(), workspace_package_version(), workspace_members() | — | 2026-10-29 |
| `tools/tests/test_release_targets.py` | 144行/5KB | tool | test_no_active_workflow_targets_unsupported_windows_architectures(), test_release_workflow_covers_every_shipped_target(), test_every_macos_release_leg_runs_on_a_runner_of_its_own_architecture() | — | 2026-10-29 |
| `tools/validate-locales.py` | 135行/5KB | tool | fail(), load_locale(), flatten() | — | 2026-10-19 |
| `tools/collect-failure-evidence.py` | 133行/4KB | entry | safe_name(), redact(), is_candidate() | — | 2026-10-14 |
| `tools/changed-paths.py` | 129行/4KB | entry | is_documentation(), docs_only(), diff_range() | — | 2026-10-14 |
| `tools/tests/test_collect_failure_evidence.py` | 125行/4KB | tool | test_collects_only_bounded_redacted_evidence(), test_skips_symlinks_and_non_validation_images(), test_unknown_log_lines_are_replaced_with_a_placeholder() | — | 2026-10-29 |
| `tools/tests/test_verify_documentation_filter.py` | 123行/5KB | tool | load_changed_paths(), jobs(), test_every_job_has_made_a_deliberate_skip_decision() | — | 2026-10-14 |
| `tools/tests/test_changed_paths.py` | 114行/4KB | tool | load(), setUp(), test_documents_alone_are_documentation() | — | 2026-10-31 |
| `tools/tests/test_swift_runtime_rpath.py` | 110行/4KB | tool | package_dependencies(), packages_linking_swift(), missing_rpath() | — | 2026-10-14 |
| `tools/tests/test_dependabot_contract.py` | 109行/4KB | tool | read(), configured_directories(), workspace_directories() | — | 2026-10-14 |
| `tools/tests/test_supported_platform_cfg.py` | 88行/3KB | tool | scan_rust_file(), offenders(), test_forbidden_windows_and_linux_are_reported() | — | 2026-10-29 |
| `tools/record-provenance.py` | 75行/2KB | entry | run(), rustc_metadata(), sha256() | — | 2026-10-14 |
| `tools/tests/test_failure_evidence_workflow.py` | 60行/2KB | tool | test_every_job_collects_redacted_failure_evidence(), test_storage_smoke_exports_preview_on_macos(), test_macos_spike_evidence_collection_runs_after_all_smokes() | — | 2026-10-29 |
| `tools/tests/test_provenance.py` | 52行/1KB | tool | test_writes_path_free_reproducible_metadata() | — | 2026-10-14 |
<!-- END GENERATED: file-table -->

---

<!-- BEGIN GENERATED: function-details -->
<details>
<summary><strong>📦 tools/</strong> (22 文件)</summary>

### tools/validate-fixtures.py

| 行号 | 函数 | 参数（前5） |
|-----:|------|-----------|
| 62 | `fail()` | path, message |
| 66 | `load()` | path |
| 76 | `validate_input()` | path, value |
| 186 | `validate_expected()` | path, value, input_id, input_events |
| 207 | `safe_package_reference()` | reference |
| 220 | `materialize_model_case()` | case, source_dir, destination |
| 261 | `parse_model_entry()` | path, fixture_path |
| 271 | `display_info_is_valid()` | path |
| 337 | `inspect_model_case()` | package_dir, fixture_path, maximum_dimension |
| 394 | `validate_model_fixtures()` |  |
| 455 | `validate_behavior_manifest()` |  |
| 508 | `main()` |  |

### tools/tests/test_release_changelog_contract.py

| 行号 | 函数 | 参数（前5） |
|-----:|------|-----------|
| 65 | `read()` | path |
| 69 | `rust_string_constant()` | source, name |
| 76 | `level_two_heading()` | line |
| 93 | `level_three_headings()` | path |
| 127 | `without_variation_selector()` | text |
| 139 | `fence_marker()` | line |
| 150 | `changelog_entries()` | path |
| 182 | `section_entry_counts()` | path |
| 212 | `class ReleaseNotesContractTests` | — |
| 393 | `class ChangelogSectionVocabularyTests` | — |
| 213 | `test_the_notes_come_from_the_changelogs_and_not_from_github()` | self |
| 229 | `test_the_release_body_and_the_manifest_read_one_notes_file()` | self |
| 252 | `test_the_notes_are_composed_before_they_are_consumed()` | self |
| 268 | `test_the_packaging_tool_reads_the_changelogs_the_repository_has()` | self |
| 287 | `test_the_bilingual_changelogs_document_the_same_releases()` | self |
| 305 | `test_the_bilingual_changelogs_carry_the_same_entries_per_section()` | self |
| 324 | `test_every_documented_release_has_notes()` | self |
| 334 | `test_the_changelogs_do_not_carry_the_generated_download_block()` | self |
| 396 | `test_the_english_headings_are_from_the_vocabulary()` | self |
| 414 | `test_the_chinese_headings_are_from_the_vocabulary()` | self |
| 432 | `test_both_changelogs_carry_the_same_sections_in_the_same_order()` | self |
| 442 | `test_the_vocabulary_does_not_repeat_a_section()` | self |
| 450 | `test_the_documented_vocabulary_is_the_enforced_one()` | self |

### tools/tests/test_packaging_contract.py

| 行号 | 函数 | 参数（前5） |
|-----:|------|-----------|
| 34 | `read()` | path |
| 38 | `shipped_targets()` |  |
| 46 | `class PackagingTargetTests` | — |
| 217 | `class MacosBundleTests` | — |
| 385 | `class BuildEntryPointTests` | — |
| 439 | `class ReleaseMatrixTests` | — |
| 47 | `test_exactly_two_release_targets_ship()` | self |
| 63 | `test_every_release_target_publishes_the_expected_artifacts()` | self |
| 70 | `test_release_jobs_and_the_release_notes_call_a_machine_the_same_thing()` | self |
| 100 | `test_every_release_job_is_named_by_the_action_it_performs()` | self |
| 129 | `test_a_step_named_require_actually_fails_the_build()` | self |
| 144 | `test_the_build_job_names_the_build_step_the_same_way()` | self |
| 151 | `test_every_release_artifact_is_keyed_by_the_architecture_token()` | self |
| 171 | `test_the_published_changelogs_call_a_chip_by_one_name()` | self |
| 185 | `test_every_macos_artifact_is_named_after_the_same_architecture_token()` | self |
| 218 | `test_bundle_identity_and_minimum_version_are_single_sourced()` | self |
| 242 | `test_bundled_resources_match_the_runtime_lookup()` | self |
| 253 | `test_bundle_is_rejected_when_the_resources_are_missing()` | self |
| 258 | `test_disk_image_wraps_the_packaged_bundle()` | self |
| 268 | `test_disk_image_declares_the_window_the_installer_opens_with()` | self |
| 303 | `test_disk_image_carries_a_repair_command_for_the_installed_copy()` | self |
| 320 | `test_the_repair_command_says_what_to_do_when_there_is_nothing_to_repair()` | self |
| 339 | `test_the_repair_command_closes_the_window_it_ran_in()` | self |
| 367 | `test_disk_image_gives_the_volume_the_application_icon()` | self |
| 386 | `test_just_stays_a_thin_entry_point()` | self |
| 403 | `test_version_has_exactly_one_source()` | self |
| 410 | `test_the_compiled_environment_reaches_the_child_build()` | self |
| 420 | `test_app_uses_a_single_production_feature()` | self |
| 426 | `test_no_self_built_packaging_script_remains()` | self |
| 440 | `triples()` | self |
| 447 | `update_targets()` | self |
| 454 | `test_release_workflow_builds_every_shipped_target()` | self |
| 460 | `test_release_workflow_runs_the_same_entry_point_as_developers()` | self |
| 466 | `test_update_runtime_and_dependency_policy_match_the_shipped_targets()` | self |
| 473 | `test_packaging_targets_match_the_update_runtime()` | self |

### tools/inspect-cubism-sdk.py

| 行号 | 函数 | 参数（前5） |
|-----:|------|-----------|
| 63 | `class InspectionError` | — |
| 68 | `class LocatedMember` | — |
| 74 | `sha256_file()` | path |
| 82 | `git_blob_sha()` | data |
| 87 | `normalized_archive_name()` | name |
| 105 | `validate_archive_entries()` | infos |
| 154 | `find_unique_suffix()` | entries, suffix, role |
| 166 | `hash_member()` | archive, info |
| 180 | `member_bytes()` | archive, info |
| 192 | `member_report()` | archive, member |
| 204 | `decode_text()` | archive, info, role |
| 211 | `find_core_root()` | entries |
| 218 | `core_member()` | entries, core_root, relative_path, role |
| 228 | `detect_framework_release()` | archive, entries |
| 239 | `validate_framework_sources()` | archive, entries, expected_blobs |
| 266 | `detect_core_version()` | core_changelog |
| 276 | `validate_redistributable_list()` | text |
| 295 | `inspect_archive()` | path, expected_sha256, framework_source_blobs |
| 403 | `parse_args()` | argv |
| 426 | `main()` | argv |

### tools/validate-json-schema.py

| 行号 | 函数 | 参数（前5） |
|-----:|------|-----------|
| 28 | `load()` | path |
| 37 | `validate_schema()` | schema_path |
| 46 | `validate_directory()` | directory, validator |
| 62 | `validate_file()` | path, validator |
| 75 | `is_portable_model_id()` | value |
| 79 | `has_control_character()` | value |
| 83 | `config_semantic_errors()` | value |
| 160 | `validate_manifest_fixtures()` | directory, validator, label |
| 216 | `main()` |  |

### tools/run-input-fixtures.py

| 行号 | 函数 | 参数（前5） |
|-----:|------|-----------|
| 15 | `load()` | path |
| 25 | `button_key()` | button |
| 29 | `parameter_names()` | sequence |
| 44 | `apply_event()` | state, event |
| 102 | `hand_state()` | state, context, side |
| 112 | `key_overlays()` | state, context |
| 142 | `parameter_value()` | name, state, context |
| 162 | `run_fixture()` | input_path |
| 217 | `main()` |  |

### tools/tests/test_update_release_contract.py

| 行号 | 函数 | 参数（前5） |
|-----:|------|-----------|
| 40 | `read()` | path |
| 44 | `section()` | source, header |
| 52 | `rust_string_constant()` | source, name |
| 59 | `runtime_constant()` | name |
| 63 | `packaging_constant()` | name |
| 67 | `manifest_platform_keys()` | source, type_name |
| 82 | `class UpdateReleaseIdentityTests` | — |
| 83 | `test_release_executable_name_matches_the_shipped_executable()` | self |
| 104 | `test_macos_bundle_name_matches_the_packaged_app()` | self |
| 118 | `test_repository_matches_the_workspace_manifest()` | self |
| 138 | `test_release_targets_match_the_dependency_policy_matrix()` | self |
| 158 | `test_manifest_platform_keys_agree_with_the_packaging_tool()` | self |
| 174 | `test_manifest_name_agrees_with_the_packaging_tool()` | self |
| 192 | `test_manifest_fragments_are_named_after_the_platform_keys()` | self |

### tools/tests/test_inspect_cubism_sdk.py

| 行号 | 函数 | 参数（前5） |
|-----:|------|-----------|
| 34 | `valid_entries()` |  |
| 59 | `write_zip()` | path, entries |
| 65 | `inspect()` | path, expected_hash |
| 73 | `class CubismSdkInspectorTests` | — |
| 74 | `test_valid_archive_reports_required_targets_and_hashes()` | self |
| 95 | `test_expected_archive_hash_is_enforced()` | self |
| 105 | `test_path_traversal_is_rejected_before_artifact_inspection()` | self |
| 117 | `test_case_colliding_paths_are_rejected()` | self |
| 129 | `test_symbolic_link_entry_is_rejected()` | self |
| 144 | `test_duplicate_path_is_rejected()` | self |
| 158 | `test_missing_required_artifact_is_rejected()` | self |
| 170 | `test_framework_release_mismatch_is_rejected()` | self |
| 182 | `test_framework_source_blob_mismatch_is_rejected()` | self |

### tools/tests/test_product_version_contract.py

| 行号 | 函数 | 参数（前5） |
|-----:|------|-----------|
| 26 | `read()` | path |
| 30 | `workspace_package_version()` |  |
| 50 | `workspace_members()` |  |
| 70 | `class ProductVersionContractTests` | — |
| 71 | `test_workspace_owns_a_valid_product_version()` | self |
| 75 | `test_workspace_members_inherit_the_product_version()` | self |
| 86 | `test_lockfile_workspace_packages_match_the_product_version()` | self |
| 108 | `test_runtime_and_ui_use_the_compiled_product_version()` | self |
| 163 | `test_macos_bundle_version_is_generated_at_package_time()` | self |
| 178 | `test_build_documentation_uses_a_version_placeholder()` | self |

### tools/tests/test_release_targets.py

| 行号 | 函数 | 参数（前5） |
|-----:|------|-----------|
| 24 | `class ReleaseTargetTests` | — |
| 25 | `test_no_active_workflow_targets_unsupported_windows_architectures()` | self |
| 32 | `test_release_workflow_covers_every_shipped_target()` | self |
| 39 | `test_every_macos_release_leg_runs_on_a_runner_of_its_own_architecture()` | self |
| 87 | `test_active_ci_builds_no_windows_target()` | self |
| 92 | `test_dependency_policy_audits_exactly_the_shipped_targets()` | self |

### tools/validate-locales.py

| 行号 | 函数 | 参数（前5） |
|-----:|------|-----------|
| 31 | `fail()` | path, message |
| 36 | `load_locale()` | path |
| 46 | `flatten()` | value, path, source |
| 69 | `placeholder_names()` | value |
| 73 | `check_ellipsis()` | locale, key, value |
| 82 | `check_single_brace_placeholder()` | locale, key, value |
| 93 | `main()` |  |

### tools/collect-failure-evidence.py

| 行号 | 函数 | 参数（前5） |
|-----:|------|-----------|
| 31 | `safe_name()` | name |
| 36 | `redact()` | value |
| 48 | `is_candidate()` | path |
| 58 | `collect()` | input_root, output_root, platform |
| 123 | `main()` |  |

### tools/changed-paths.py

| 行号 | 函数 | 参数（前5） |
|-----:|------|-----------|
| 46 | `is_documentation()` | path |
| 61 | `docs_only()` | changed |
| 70 | `diff_range()` | event_name, base_ref |
| 86 | `changed_paths()` | event_name, base_ref |
| 100 | `main()` |  |

### tools/tests/test_collect_failure_evidence.py

| 行号 | 函数 | 参数（前5） |
|-----:|------|-----------|
| 13 | `class FailureEvidenceTests` | — |
| 14 | `test_collects_only_bounded_redacted_evidence()` | self |
| 49 | `test_skips_symlinks_and_non_validation_images()` | self |
| 74 | `test_unknown_log_lines_are_replaced_with_a_placeholder()` | self |
| 100 | `test_output_directory_inside_input_is_not_collected()` | self |

### tools/tests/test_verify_documentation_filter.py

| 行号 | 函数 | 参数（前5） |
|-----:|------|-----------|
| 21 | `load_changed_paths()` |  |
| 45 | `jobs()` |  |
| 49 | `class DocumentationFilterTests` | — |
| 50 | `test_every_job_has_made_a_deliberate_skip_decision()` | self |
| 69 | `test_the_filter_job_publishes_the_output_the_others_read()` | self |
| 75 | `test_the_workflow_always_runs_so_a_skipped_job_still_reports()` | self |
| 98 | `test_the_filter_keeps_its_own_inputs_out_of_the_documentation_set()` | self |
| 113 | `test_no_job_is_left_reachable_only_through_a_manual_trigger()` | self |

### tools/tests/test_changed_paths.py

| 行号 | 函数 | 参数（前5） |
|-----:|------|-----------|
| 14 | `load()` |  |
| 21 | `class DocumentationOnlyTests` | — |
| 22 | `setUp()` | self |
| 25 | `test_documents_alone_are_documentation()` | self |
| 42 | `test_a_change_set_of_only_documents_is_docs_only()` | self |
| 49 | `test_one_code_path_is_enough_to_run_everything()` | self |
| 74 | `test_the_gate_configuration_is_never_treated_as_a_document()` | self |
| 89 | `test_an_unreadable_change_set_runs_the_whole_pipeline()` | self |
| 97 | `test_the_diff_range_follows_the_event()` | self |
| 106 | `test_a_pull_request_without_its_base_ref_is_an_error()` | self |

### tools/tests/test_swift_runtime_rpath.py

| 行号 | 函数 | 参数（前5） |
|-----:|------|-----------|
| 33 | `package_dependencies()` | crate |
| 44 | `packages_linking_swift()` | root |
| 60 | `missing_rpath()` | root |
| 76 | `class SwiftRuntimeRpathContractTests` | — |
| 77 | `test_every_package_linking_swift_carries_the_rpath()` | self |
| 81 | `test_the_scan_finds_the_packages_it_is_supposed_to()` | self |
| 93 | `test_a_build_script_without_the_flag_is_reported()` | self |

### tools/tests/test_dependabot_contract.py

| 行号 | 函数 | 参数（前5） |
|-----:|------|-----------|
| 25 | `read()` | path |
| 29 | `configured_directories()` |  |
| 47 | `workspace_directories()` |  |
| 61 | `class DependabotContractTests` | — |
| 62 | `test_the_configuration_has_one_cargo_update_entry()` | self |
| 71 | `test_every_configured_directory_exists_and_holds_a_manifest()` | self |
| 87 | `test_every_independent_workspace_is_monitored()` | self |
| 98 | `test_entries_use_repository_relative_paths()` | self |

### tools/tests/test_supported_platform_cfg.py

| 行号 | 函数 | 参数（前5） |
|-----:|------|-----------|
| 21 | `scan_rust_file()` | path |
| 39 | `offenders()` | root |
| 47 | `class SupportedPlatformCfgDetectorTests` | — |
| 81 | `class SupportedPlatformCfgContractTests` | — |
| 48 | `test_forbidden_windows_and_linux_are_reported()` | self |
| 66 | `test_supported_cfgs_pass()` | self |
| 82 | `test_crates_contain_no_unsupported_platform_cfg()` | self |

### tools/record-provenance.py

| 行号 | 函数 | 参数（前5） |
|-----:|------|-----------|
| 13 | `run()` |  |
| 17 | `rustc_metadata()` |  |
| 33 | `sha256()` | path |
| 41 | `main()` |  |

### tools/tests/test_failure_evidence_workflow.py

| 行号 | 函数 | 参数（前5） |
|-----:|------|-----------|
| 10 | `class FailureEvidenceWorkflowTests` | — |
| 11 | `test_every_job_collects_redacted_failure_evidence()` | self |
| 41 | `test_storage_smoke_exports_preview_on_macos()` | self |
| 50 | `test_macos_spike_evidence_collection_runs_after_all_smokes()` | self |

### tools/tests/test_provenance.py

| 行号 | 函数 | 参数（前5） |
|-----:|------|-----------|
| 13 | `class ProvenanceTests` | — |
| 14 | `test_writes_path_free_reproducible_metadata()` | self |

</details>
<!-- END GENERATED: function-details -->

---

<!-- BEGIN GENERATED: cli-usage -->
### tools/changed-paths.py

```bash
python3 tools/changed-paths.py

# 参数：
    --base                               # revision range base, overriding the one derived from the event
    --changed                            # newline separated paths, instead of diffing; for tests
```

### tools/collect-failure-evidence.py

```bash
python3 tools/collect-failure-evidence.py

# 参数：
    --input                              # 
    --output                             # 
    --platform                           # 
```

### tools/inspect-cubism-sdk.py

```bash
python3 tools/inspect-cubism-sdk.py

# 参数：
    sdk_zip                              # path to CubismSdkForNative-5-r.5.zip
    --expected-sha256                    # reject the archive unless its SHA-256 matches this value
```

### tools/record-provenance.py

```bash
python3 tools/record-provenance.py

# 参数：
    --workspace                          # 
    --output                             # 
    --target                             # target triple; defaults to rustc host
    --profile                            # 
    --features                           # 
    --environment                        # 
```
<!-- END GENERATED: cli-usage -->

---

<!-- BEGIN GENERATED: edit-guide -->
| 你要做什么 | 改哪个文件 |
|-----------|-----------|
| 修改 `tools/` 相关逻辑 | `tools/validate-fixtures.py` |
| 修改运行流程/编排（changed-paths） | `tools/changed-paths.py` |
| 修改运行流程/编排（collect-failure-evidence） | `tools/collect-failure-evidence.py` |
| 修改运行流程/编排（inspect-cubism-sdk） | `tools/inspect-cubism-sdk.py` |
| 修改运行流程/编排（record-provenance） | `tools/record-provenance.py` |
<!-- END GENERATED: edit-guide -->

---

<!-- BEGIN GENERATED: env-deps -->
> ⚠️ 未在项目根找到 `pyproject.toml`/`requirements.txt` 的声明依赖，或依赖表为空。仅做本地静态读取，不做联网检测。
<!-- END GENERATED: env-deps -->
