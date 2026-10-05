#!/bin/bash
# ============================================================
#  跑跑看：虛擬風洞 vortexlab
#
#  這個檔案在 Finder 裡雙擊就會打開「終端機」來執行。它會做四件事：
#    1. 用 Rust 編譯程式（第一次大約半分鐘，之後幾乎不用等）。
#    2. 跑一個示範（大約半分鐘）：
#       先算一題流體力學的「標準考題」（管道裡的圓柱繞流），
#       把算出來的數字跟論文公布的參考區間排在一起給你看；
#       再算一次圓柱後面的「渦街」，存成動畫。
#    3. 問你要不要把自己畫的形狀放進風洞。
#       把黑白 PNG 圖片放進「我的形狀」資料夾就可以（深色是物體、淺色是空氣，
#       風從左邊吹過來）。資料夾裡已經有兩個範例：台北101平面.png、機翼.png。
#    4. 用瀏覽器打開完整的驗證報告（docs/report/index.html）。
#
#  需要先裝好 Rust（提供 cargo）：到 https://rustup.rs 照指示安裝。
#  其他什麼都不用裝；程式不連網路、不開任何連接埠、結束後不留任何東西在背景。
# ============================================================

# 先切換到這個檔案所在的資料夾。資料夾名稱有空白和中文，
# 所以 "$(dirname "$0")" 一定要用雙引號包起來。
cd "$(dirname "$0")" || exit 1

pause_and_exit() {
  read -r -p "按 Enter 關閉視窗..."
  exit "$1"
}

# Homebrew 安裝的 rustup 不一定在 PATH 裡，先幫忙加上。
if ! command -v cargo >/dev/null 2>&1; then
  for d in "$HOME/.cargo/bin" /opt/homebrew/opt/rustup/bin; do
    if [ -x "$d/cargo" ]; then
      export PATH="$d:$PATH"
      break
    fi
  done
fi
if ! command -v cargo >/dev/null 2>&1; then
  echo "找不到 cargo（Rust）。請打開 https://rustup.rs ，照網頁上的一行指令安裝後再試一次。"
  pause_and_exit 1
fi

export VORTEXLAB_LANG=zh
BIN=./target/release/vortexlab
THREADS=4

echo "== 1/4 編譯（Rust，release 模式，最多同時用 4 個 CPU 核心）..."
# 編譯訊息很長，只留最後幾行；pipefail 讓「cargo 失敗」不會被 tail 蓋掉。
set -o pipefail
if ! cargo build --release -j 4 2>&1 | tail -3 || [ ! -x "$BIN" ]; then
  echo "編譯失敗，上面的訊息會說明原因。"
  pause_and_exit 1
fi
set +o pipefail

echo
echo "== 2/4 示範：圓柱繞流"
echo "   Cd 是阻力係數、Cl 是升力係數、St（史特豪數）是渦流脫落的頻率，"
echo "   D/U 是時間單位（氣流走過一個物體寬度所花的時間）。"
echo
if ! "$BIN" demo --threads "$THREADS" --out "results/demo"; then
  echo "示範執行失敗。"
  pause_and_exit 1
fi
echo
echo "   動畫在 results/demo/vorticity.gif（藍色順時針、紅色逆時針的漩渦）。"
open "results/demo/vorticity.gif" 2>/dev/null

echo
echo "== 3/4 放自己的形狀進風洞"
shopt -s nullglob
shapes=("我的形狀"/*.png)
shopt -u nullglob
if [ ${#shapes[@]} -eq 0 ]; then
  echo "   「我的形狀」資料夾裡沒有 PNG 圖片，跳過這一步。"
else
  echo "   「我的形狀」資料夾裡有這些圖片："
  i=1
  for f in "${shapes[@]}"; do
    echo "     $i) $(basename "$f")"
    i=$((i + 1))
  done
  read -r -p "   輸入編號後按 Enter 就開始算（大約一分鐘）；直接按 Enter 跳過： " pick
  if [[ "$pick" =~ ^[0-9]+$ ]] && [ "$pick" -ge 1 ] && [ "$pick" -le ${#shapes[@]} ]; then
    chosen="${shapes[$((pick - 1))]}"
    name="$(basename "$chosen" .png)"
    echo
    if "$BIN" shape "$chosen" --re 150 --cells 48 --units 80 --threads "$THREADS"; then
      echo
      echo "   結果在「我的形狀/輸出/$name/」：wake.png（尾流圖）、vorticity.gif（動畫）、forces.csv（受力紀錄）。"
      open "我的形狀/輸出/$name/vorticity.gif" 2>/dev/null
    else
      echo "   這張圖沒有算成功，原因寫在上面。"
    fi
  else
    echo "   跳過。之後想試的話，在終端機執行："
    echo "     $BIN shape 我的形狀/你的圖.png"
  fi
fi

echo
echo "== 4/4 打開驗證報告"
if [ -f docs/report/index.html ]; then
  open docs/report/index.html
  echo "   已用瀏覽器打開 docs/report/index.html"
  echo "   （裡面是每一題驗證的數字、論文參考區間，還有台北 101 鋸齒角的實驗）"
else
  echo "   找不到 docs/report/index.html。"
fi
echo
echo "想知道每個檔案在做什麼，請看 docs/導讀.zh-TW.md。"
pause_and_exit 0
