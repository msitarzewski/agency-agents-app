#!/usr/bin/env bash
#
# Phase C build/validation runner.
#
# Default:
#   npm run build:phase-c
#
# Full local + Parallels VM matrix:
#   npm run build:phase-c:full
#
# Useful overrides:
#   AGENCY_AGENTS_PARITY_ROOT=/path/to/agency-agents npm run build:phase-c
#   PHASE_C_SKIP_MAC_BUILD=1 npm run build:phase-c
#   PHASE_C_INCLUDE_VMS=1 PHASE_C_UBUNTU_VM=Scratch PHASE_C_WINDOWS_VM="Windows 11" npm run build:phase-c

set -u -o pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
RUN_ID="$(date -u +"%Y%m%dT%H%M%SZ")"
OUT_DIR="$ROOT/.phase-c/runs/$RUN_ID"
LOG_DIR="$OUT_DIR/logs"
SCREEN_DIR="$OUT_DIR/screenshots"
REPORT_MD="$OUT_DIR/phase-c-report.md"
REPORT_JSONL="$OUT_DIR/phase-c-results.jsonl"

PARITY_ROOT="${AGENCY_AGENTS_PARITY_ROOT:-/Users/michael/Software/AgentLand/agency-agents}"
INCLUDE_VMS="${PHASE_C_INCLUDE_VMS:-0}"
SKIP_MAC_BUILD="${PHASE_C_SKIP_MAC_BUILD:-0}"
STRICT_TAURI="${PHASE_C_STRICT_TAURI:-0}"
UBUNTU_VM="${PHASE_C_UBUNTU_VM:-Scratch}"
WINDOWS_VM="${PHASE_C_WINDOWS_VM:-Windows 11}"

PASS_COUNT=0
FAIL_COUNT=0
SKIP_COUNT=0

mkdir -p "$LOG_DIR" "$SCREEN_DIR"

json_escape() {
  python3 -c 'import json,sys; print(json.dumps(sys.stdin.read()))'
}

emit_jsonl() {
  local status="$1"
  local name="$2"
  local log="$3"
  local duration="$4"
  local name_json log_json
  name_json="$(printf '%s' "$name" | json_escape)"
  log_json="$(printf '%s' "$log" | json_escape)"
  printf '{"status":"%s","name":%s,"log":%s,"durationSeconds":%s}\n' "$status" "$name_json" "$log_json" "$duration" >> "$REPORT_JSONL"
}

append_md() {
  printf '%s\n' "$*" >> "$REPORT_MD"
}

start_report() {
  cat > "$REPORT_MD" <<EOF
# Phase C Report

- Run ID: \`$RUN_ID\`
- Repo: \`$ROOT\`
- Catalog parity root: \`$PARITY_ROOT\`
- Include VMs: \`$INCLUDE_VMS\`

## Results

EOF
  : > "$REPORT_JSONL"
}

run_step() {
  local name="$1"
  shift
  local slug
  slug="$(printf '%s' "$name" | tr '[:upper:]' '[:lower:]' | tr -cs 'a-z0-9' '-' | sed 's/^-//; s/-$//')"
  local log="$LOG_DIR/${slug}.log"
  local start end status

  printf '==> %s\n' "$name"
  start="$(date +%s)"
  (
    cd "$ROOT"
    printf '$'
    printf ' %q' "$@"
    printf '\n\n'
    "$@"
  ) >"$log" 2>&1
  status=$?
  end="$(date +%s)"

  if [[ "$status" -eq 0 ]]; then
    PASS_COUNT=$((PASS_COUNT + 1))
    append_md "- PASS: $name ([log](logs/$(basename "$log")))"
    emit_jsonl "pass" "$name" "logs/$(basename "$log")" "$((end - start))"
    printf '    PASS\n'
  else
    FAIL_COUNT=$((FAIL_COUNT + 1))
    append_md "- FAIL: $name ([log](logs/$(basename "$log")))"
    emit_jsonl "fail" "$name" "logs/$(basename "$log")" "$((end - start))"
    printf '    FAIL — see %s\n' "$log"
  fi
}

skip_step() {
  local name="$1"
  local reason="$2"
  SKIP_COUNT=$((SKIP_COUNT + 1))
  append_md "- SKIP: $name — $reason"
  emit_jsonl "skip" "$name" "$reason" 0
  printf '==> %s\n    SKIP — %s\n' "$name" "$reason"
}

run_local_phase_c() {
  run_step "git status baseline" git status --short
  run_step "phase c config validation" node tools/phase-c/validate-config.mjs

  if [[ -d "$PARITY_ROOT" && ( -x "$PARITY_ROOT/convert.sh" || -x "$PARITY_ROOT/scripts/convert.sh" ) ]]; then
    run_step "renderer parity against active catalog" env AGENCY_AGENTS_PARITY_ROOT="$PARITY_ROOT" cargo test --manifest-path src-tauri/Cargo.toml --lib upstream_convert_sh_is_byte_identical_for_transform_tools -- --ignored --nocapture
  else
    skip_step "renderer parity against active catalog" "missing executable convert.sh at $PARITY_ROOT"
  fi

  run_step "rust library tests" cargo test --manifest-path src-tauri/Cargo.toml --lib
  run_step "frontend type check" npm run check
  run_step "frontend production build" npm run build

  if [[ "$SKIP_MAC_BUILD" == "1" ]]; then
    skip_step "macOS rust release build" "PHASE_C_SKIP_MAC_BUILD=1"
  else
    run_step "macOS rust release build" env CARGO_TARGET_DIR="$OUT_DIR/target/macos-release" TAURI_CONFIG='{"app":{"macOSPrivateApi":false}}' cargo build --manifest-path src-tauri/Cargo.toml --release
  fi

  if [[ "$STRICT_TAURI" == "1" ]]; then
    run_step "macOS tauri no-bundle build strict" env CARGO_TARGET_DIR="$OUT_DIR/target/macos-tauri-release" npm run tauri -- build --no-bundle
  else
    skip_step "macOS tauri no-bundle build strict" "set PHASE_C_STRICT_TAURI=1"
  fi
}

run_ubuntu_vm() {
  if ! command -v prlctl >/dev/null 2>&1; then
    skip_step "ubuntu parallels smoke" "prlctl not found"
    return
  fi

  run_step "ubuntu vm sync repo" prlctl exec "$UBUNTU_VM" bash -lc 'set -euo pipefail; src=""; for d in /media/psf/agency-agents-app /mnt/psf/agency-agents-app /media/sf_agency-agents-app; do if [ -d "$d" ]; then src="$d"; break; fi; done; test -n "$src"; mkdir -p /root/agency-agents-app; rsync -a --delete --exclude .git --exclude node_modules --exclude .svelte-kit --exclude build --exclude .phase-c --exclude target --exclude src-tauri/target "$src"/ /root/agency-agents-app/'
  run_step "ubuntu vm npm install" prlctl exec "$UBUNTU_VM" npm --prefix /root/agency-agents-app ci
  run_step "ubuntu vm cargo tests" prlctl exec "$UBUNTU_VM" cargo test --manifest-path /root/agency-agents-app/src-tauri/Cargo.toml --lib
  run_step "ubuntu vm frontend check" prlctl exec "$UBUNTU_VM" npm --prefix /root/agency-agents-app run check
  run_step "ubuntu vm frontend build" prlctl exec "$UBUNTU_VM" npm --prefix /root/agency-agents-app run build
  run_step "ubuntu vm tauri bundle" bash -c 'prlctl exec "$1" npm --prefix /root/agency-agents-app run tauri -- build || true' phase-c "$UBUNTU_VM"
  run_step "ubuntu vm bundle artifact check" prlctl exec "$UBUNTU_VM" test -d /root/agency-agents-app/src-tauri/target/release/bundle/deb -a -d /root/agency-agents-app/src-tauri/target/release/bundle/rpm -a -d /root/agency-agents-app/src-tauri/target/release/bundle/appimage

  if prlctl capture "$UBUNTU_VM" --file "$SCREEN_DIR/ubuntu.png" >"$LOG_DIR/ubuntu-capture.log" 2>&1; then
    append_md "- ARTIFACT: Ubuntu screenshot [ubuntu.png](screenshots/ubuntu.png)"
  fi
}

run_windows_vm() {
  if ! command -v prlctl >/dev/null 2>&1; then
    skip_step "windows parallels matrix" "prlctl not found"
    return
  fi

  local win_env='set "CARGO_HOME=C:\CargoSystem" & set "RUSTUP_HOME=C:\Windows\System32\config\systemprofile\.rustup" & set "PATH=C:\Users\michael\.nvm\versions\node\v24.11.1\bin;C:\BuildTools\VC\Tools\Llvm\bin;C:\Windows\System32\config\systemprofile\.cargo\bin;%PATH%"'

  run_step "windows vm sync repo" prlctl exec "$WINDOWS_VM" cmd.exe /c 'robocopy \\Mac\agency-agents-app C:\AgencyAgentsWinTest /MIR /XD .git node_modules .svelte-kit build .phase-c target src-tauri\target /XF .DS_Store Thumbs.db /NFL /NDL /NJH /NJS /NP & if %ERRORLEVEL% LEQ 7 exit /B 0 else exit /B %ERRORLEVEL%'
  run_step "windows vm rust target setup" prlctl exec "$WINDOWS_VM" cmd.exe /c "set \"PATH=C:\\Windows\\System32\\config\\systemprofile\\.cargo\\bin;%PATH%\" & rustup target add x86_64-pc-windows-msvc"
  run_step "windows vm npm install" prlctl exec "$WINDOWS_VM" cmd.exe /c "set \"PATH=C:\\Users\\michael\\.nvm\\versions\\node\\v24.11.1\\bin;%PATH%\" & cd /d C:\\AgencyAgentsWinTest & npm ci"
  run_step "windows arm64 build" prlctl exec "$WINDOWS_VM" cmd.exe /c "call C:\\BuildTools\\VC\\Auxiliary\\Build\\vcvarsall.bat arm64 & $win_env & set \"CARGO_TARGET_DIR=C:\\AgencyAgentsWinTarget\" & cd /d C:\\AgencyAgentsWinTest & npm run tauri -- build --no-bundle"
  run_step "windows x64 build" prlctl exec "$WINDOWS_VM" cmd.exe /c "call C:\\BuildTools\\VC\\Auxiliary\\Build\\vcvarsall.bat x64 & $win_env & set \"CARGO_TARGET_DIR=C:\\AgencyAgentsWinTargetX64\" & cd /d C:\\AgencyAgentsWinTest & npm run tauri -- build --no-bundle --target x86_64-pc-windows-msvc"
  run_step "windows pe header verification" prlctl exec "$WINDOWS_VM" cmd.exe /c "call C:\\BuildTools\\VC\\Auxiliary\\Build\\vcvarsall.bat x64 & dumpbin /headers C:\\AgencyAgentsWinTarget\\release\\agency-agents-app.exe | findstr /C:\"AA64 machine\" & dumpbin /headers C:\\AgencyAgentsWinTargetX64\\x86_64-pc-windows-msvc\\release\\agency-agents-app.exe | findstr /C:\"8664 machine\""
  run_step "windows named artifacts" prlctl exec "$WINDOWS_VM" cmd.exe /c "if not exist C:\\AgencyAgentsArtifacts mkdir C:\\AgencyAgentsArtifacts & copy /Y C:\\AgencyAgentsWinTarget\\release\\agency-agents-app.exe \"C:\\AgencyAgentsArtifacts\\Agency Agents-0.1.0-windows-arm64.exe\" & copy /Y C:\\AgencyAgentsWinTargetX64\\x86_64-pc-windows-msvc\\release\\agency-agents-app.exe \"C:\\AgencyAgentsArtifacts\\Agency Agents-0.1.0-windows-x64.exe\" & dir C:\\AgencyAgentsArtifacts"

  if prlctl capture "$WINDOWS_VM" --file "$SCREEN_DIR/windows.png" >"$LOG_DIR/windows-capture.log" 2>&1; then
    append_md "- ARTIFACT: Windows screenshot [windows.png](screenshots/windows.png)"
  fi
}

finish_report() {
  cat >> "$REPORT_MD" <<EOF

## Summary

- Passed: $PASS_COUNT
- Failed: $FAIL_COUNT
- Skipped: $SKIP_COUNT

## Notes

- Full VM validation is opt-in via \`PHASE_C_INCLUDE_VMS=1\` or \`npm run build:phase-c:full\`.
- The default macOS gate uses \`cargo build --release\` with a non-packaging \`TAURI_CONFIG\` override; real platform config is checked separately by \`validate-config.mjs\`.
- The Tauri CLI no-bundle smoke is opt-in via \`PHASE_C_STRICT_TAURI=1\`.
- The Ubuntu bundle command may log an updater-signing warning when VM signing keys are absent; the artifact check is the pass/fail gate for Linux packaging.
- VM steps assume the prepared Parallels guests and paths used during Phase C:
  - Ubuntu: \`$UBUNTU_VM\`, repo at \`/root/agency-agents-app\`
  - Windows: \`$WINDOWS_VM\`, repo at \`C:\\AgencyAgentsWinTest\`
- Reports are intentionally ignored by git under \`.phase-c/\`.
EOF
}

start_report
run_local_phase_c

if [[ "$INCLUDE_VMS" == "1" ]]; then
  run_ubuntu_vm
  run_windows_vm
else
  skip_step "ubuntu parallels smoke" "set PHASE_C_INCLUDE_VMS=1"
  skip_step "windows parallels matrix" "set PHASE_C_INCLUDE_VMS=1"
fi

finish_report

printf '\nPhase C report: %s\n' "$REPORT_MD"
printf 'Results: %s passed, %s failed, %s skipped\n' "$PASS_COUNT" "$FAIL_COUNT" "$SKIP_COUNT"

if [[ "$FAIL_COUNT" -gt 0 ]]; then
  exit 1
fi
