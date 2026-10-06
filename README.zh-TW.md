# vortexlab 虛擬風洞

用軟體做的二維小風洞：從零用 Rust 寫的格子波茲曼（lattice-Boltzmann）流體解算器，沒有用任何外部套件；
**像檢驗工程工具一樣，拿論文公布的標準數字來對答案**；最後用它做一個實驗：台北 101 的「鋸齒狀」內凹角，
對甩動方形高樓的漩渦有什麼影響。

瀏覽器上的流體小玩具到處都是，這個方法也不是新的；正式的格子波茲曼程式列在[相關作品](#相關作品)。
這個專案多做的是「檢查」：下面每一個結果，都附上它比對的參考值、在每一種網格解析度下落在參考區間的裡面還是外面，
以及重現它的指令。

English: [README.md](README.md) · 白話導讀（給初學者）：[docs/導讀.zh-TW.md](docs/導讀.zh-TW.md)

| Check | Reference | Result (finest grid) |
|---|---|---|
| Channel flow, exact parabola | analytic | observed order 2.000; exact to rounding (10⁻¹¹ or better) with TRT, Λ = 3/16 |
| Lid-driven cavity, Re = 1000 | Botella & Peyret (1998), spectral | centreline extrema within 0.20 % at 257 × 257 |
| Lid-driven cavity, Re = 100, 1000 | Ghia, Ghia & Shin (1982) table | max deviation 0.008 / 0.016 of the lid speed, located where Ghia's table itself is 2-3 % off the spectral values |
| Cylinder, steady, Re = 20 (2D-1) | Schäfer & Turek (1996) intervals | c_D, c_L, ΔP inside; recirculation length 0.2 % below its interval |
| Cylinder, periodic, Re = 100 (2D-2) | Schäfer & Turek (1996) intervals | c_Dmax, c_Lmax, St, ΔP inside at 80 cells per diameter, at lattice velocity 0.05 and at half of it; drag oscillation amplitude Mach-dependent by a few per cent even at Ma 0.02; diverges at 10 |
| Same result for any thread count | — | bit-identical populations and forces with 1, 2, 3, 4, 7 threads |

（表格欄位依序是：檢查項目、參考來源、最細網格的結果。）完整的表格、收斂圖、受力歷程和動畫都在自動產生的報告
**[docs/report/index.html](docs/report/index.html)**（純靜態網頁，沒有任何程式碼，用瀏覽器直接打開檔案即可）。

![雙層內凹截面後方的渦街](docs/report/corners_main_double-recessed.gif)

## 示範

在 macOS 上雙擊 **`跑跑看.command`**（或執行 `make demo`）：先在粗網格上跑 Schäfer–Turek 圓柱標準考題，
再算一段渦街並存成動畫。以下是真實輸出（英文訊息；雙擊 `跑跑看.command` 時會顯示中文）：

```
$ ./target/release/vortexlab demo
[1/2] Benchmark: flow past a cylinder in a channel, Schafer & Turek (1996) case 2D-2, Re = 100
      20 cells per diameter (coarse on purpose, so it finishes in seconds).
      quantity     computed     published interval
      c_D max        3.3244       3.2200 - 3.2400   above
      c_L max        0.9807       0.9900 - 1.0100   below
      St             0.2988       0.2950 - 0.3050   inside
      dP             2.4699       2.4600 - 2.5000   inside
      (5 s; finer grids land closer: see the report)

[2/2] Vortex street behind a circular cylinder in a uniform stream, Re = 150
   20%  t =   18.0 D/U   Cd =  1.201   Cl = +0.084
   40%  t =   36.0 D/U   Cd =  1.408   Cl = -0.503
   60%  t =   54.0 D/U   Cd =  1.455   Cl = +0.584
   80%  t =   72.0 D/U   Cd =  1.456   Cl = -0.581
  100%  t =   90.0 D/U   Cd =  1.454   Cl = +0.568
Result (after the start-up transient):
  mean drag coefficient              Cd = 1.456
  mean lift coefficient              Cl = -0.009
  RMS lift fluctuation               Cl' = 0.416
  Strouhal number (lift spectrum)    St = 0.1954  (from zero crossings: 0.1954)
  a 1 cm wide body at this Reynolds number: air 0.225 m/s, drag 4.422e-4 N/m, shedding at 4.40 Hz
  a 1 cm wide body at this Reynolds number: water 0.015 m/s, drag 1.638e-3 N/m, shedding at 0.29 Hz
  grid 662 x 302 = 199924 cells, 35660 steps, 19 s, 371 MLUPS
animation and picture written to results/demo
```

### 放自己的形狀進風洞

任何「淺色（或透明）背景、深色形狀」的 PNG 都可以，風從左邊吹來。

```
$ ./target/release/vortexlab shape 我的形狀/機翼.png --re 150 --cells 60 --units 100
picture 520x220 px -> body 15 cells across the flow, length 3.98 D; Re = 150, Ma = 0.139, tau = 0.5240
   ...
  100%  t =   99.9 D/U   Cd =  1.082   Cl = +1.958
Result (after the start-up transient):
  mean drag coefficient              Cd = 1.082
  mean lift coefficient              Cl = +1.879
  RMS lift fluctuation               Cl' = 0.085
  Strouhal number (lift spectrum)    St = 0.1815  (from zero crossings: 0.1820)
  a 1 cm wide body at this Reynolds number: air 0.225 m/s, drag 3.288e-4 N/m, shedding at 4.08 Hz
  a 1 cm wide body at this Reynolds number: water 0.015 m/s, drag 1.218e-3 N/m, shedding at 0.27 Hz
  grid 347 x 152 = 52744 cells, 19780 steps, 4 s, 280 MLUPS
wake.png, vorticity.gif and forces.csv written to 我的形狀/輸出/機翼
```

程式會在圖片旁邊寫出 `wake.png`（尾流圖）、`vorticity.gif`（一個脫落週期的循環動畫）和 `forces.csv`（受力紀錄）。
選項：`--re` 雷諾數、`--cells` 形狀較長那一邊切成幾格、`--angle` 旋轉角度、`--units` 模擬多久（單位是 D/U，
氣流走過一個物體寬度的時間）、`--u` 格子速度（馬赫數 = 1.73 u）。讀 PNG（含 DEFLATE 解壓縮）和寫 GIF
的程式都是這個專案自己寫的。`我的形狀/` 裡有兩個範例。

## 運作原理

```
 populations f_0..f_8 at every node (D2Q9)          one time step
 ┌──────────────────────────────────────┐   1. bulk nodes, in parallel: pull the nine populations from the
 │ 6  2  5      stream: f_i moves one   │      upstream neighbours in array A, collide (BGK or TRT), write B
 │  \ | /       cell along c_i          │   2. boundary nodes, serial, fixed order: for every link that ends
 │ 3--0--1      collide: relax towards  │      in a wall / inlet / outlet, build the returning population
 │  / | \       local equilibrium       │      (interpolated bounce-back, moving wall, pressure), collide,
 │ 7  4  8                              │      add the momentum handed to the body to its force
 └──────────────────────────────────────┘   3. swap A and B
```

* **碰撞**（`src/lattice.rs`）：BGK 與雙鬆弛時間（TRT）。TRT 取 Λ = 3/16 時，反彈邊界的牆面剛好落在兩格正中間；
  正式計算都用它。
* **彎曲的物體**（`src/geometry.rs`、`src/sim.rs`）：每一條被物體切到的格子連結，用二分法找出「連結有多少比例在
  流體裡」（q），再用 Bouzidi 等人（2001）的內插反彈規則，所以圓就是圓，不是樓梯。只要能回答
  「這個點在不在裡面」的東西都能當形狀，包括一張圖片經雙線性內插後的 0.5 等值線。
* **受力**：在被切到的連結上做動量交換，採用 Wen 等人（2014）的伽利略不變形式。
* **史特豪數**（`src/signal.rs`）：對升力歷程做自己寫的 FFT（Hann 窗、補零、拋物線內插找峰值），
  並用過零點的平均間隔交叉檢查。
* **開放邊界**：速度入口；出口和側邊界會讓聲波穿出去。沒有這個處理，風洞會像一支風琴管一樣共鳴，
  阻力讀數會晃 ±10%；怎麼發現、怎麼修，見 [docs/DESIGN.md](docs/DESIGN.md) 第 3.2 節。
* **可重現**：每個格點的新值只看舊陣列；所有要累加的量都在單一執行緒、固定順序下累加，
  所以結果跟執行緒數量無關，連最後一個位元都一樣。

### 單位換算

解算器內部用格子單位（一格 = 1、一步 = 1）。把一次計算對應到真實流動只需要兩個選擇：物體寬度切成 `D` 格，
以及用格子速度 `U` 代表真實的來流速度。於是 `dx = D_真實 / D`、`dt = dx · U / U_真實`，黏度由雷諾數決定：
`nu = U D / Re`、`tau = 3 nu + 1/2`。有兩個限制：馬赫數 `Ma = U √3` 要小，因為這個方法的可壓縮誤差與 Ma² 成正比
（這裡用 Ma = 0.02–0.17，每張驗證表都有把馬赫數減半、2D-2 還有減成四分之一的列來顯示影響）；`tau` 不能太接近 1/2
（低於約 0.51 時網格解析不了速度梯度，可能算爆）。換算寫在 `src/units.rs`；命令列也會把每個結果換算成
「1 公分寬的物體在空氣中、在水中」的實際數字。

## 驗證

`make validate` 重現全部題目，`make report` 產生報告。下面的表是在一台 4 核心的雲端 Linux 虛擬機
（Intel Xeon 2.1 GHz，每核心一個執行緒，15 GB 記憶體）上重新算的：完整一組（包含這一輪新增的三個低速 2D-2 題）
以 4 執行緒花了 3582 秒，其中 2D-2 佔 2857 秒（80 格的兩題：U = 0.05 是 679 秒，U = 0.025 是 1367 秒）。
表裡原本就有的數值（先前在 Apple M5 上算的，幾乎相同的清單花 692 秒）在虛擬機上重算，顯示出來的位數全部相同。
參考值與出處在 [`data/reference/`](data/reference) 和 [docs/REFERENCES.md](docs/REFERENCES.md)。
✓ = 落在論文的參考區間內，✗ = 落在外面（括號內是離區間多遠）。

### 管道流（Poiseuille）對照精確解

| scheme | rows 8 → 128, relative L2 error | observed order |
|---|---|---|
| BGK, halfway wall | 1.1e-2 → 4.3e-5 | 2.000 |
| TRT Λ = 1/4, halfway wall | 7.1e-3 → 2.8e-5 | 2.000 |
| TRT Λ = 3/16, halfway wall | 3e-15 … 2e-11 | exact (rounding) |
| TRT Λ = 3/16, wall at q = 0.25 (interpolated) | 3.4e-2 → 1.1e-4 | 2.003 |
| TRT Λ = 3/16, wall at q = 0.80 (interpolated) | 1.5e-2 → 6.9e-5 | 1.997 |

用動量交換法量到的牆面阻力，與驅動流體的總力相等到 10 位數。

### 頂蓋驅動方腔

與 Ghia 表 I、II 的 2 × 15 個內部點的最大偏差，以及速度剖面極值相對於 Botella 與 Peyret 譜方法結果的差：

| Re | grid | max dev. from Ghia (u, v) | u_min | v_max | v_min |
|---|---|---|---|---|---|
| 100 | 65² | 0.0048, 0.0079 | −0.21354 (−0.24 %) | 0.17882 (−0.42 %) | −0.25245 (−0.53 %) |
| 100 | 129² | 0.0050, 0.0084 | −0.21384 (−0.10 %) | 0.17918 (−0.22 %) | −0.25301 (−0.31 %) |
| 1000 | 65² | 0.0167, 0.0087 | −0.37995 (−2.22 %) | 0.36766 (−2.46 %) | −0.51217 (−2.83 %) |
| 1000 | 129² | 0.0045, 0.0115 | −0.38676 (−0.47 %) | 0.37501 (−0.51 %) | −0.52401 (−0.58 %) |
| 1000 | 257² | 0.0061, 0.0158 | −0.38798 (−0.15 %) | 0.37638 (−0.15 %) | −0.52600 (−0.20 %) |
| 100 | Ghia 129² | — | −0.21090 (−1.47 %) | 0.17527 (−2.40 %) | −0.24533 (−3.34 %) |
| 1000 | Ghia 129² | — | −0.38289 (−1.46 %) | 0.37095 (−1.59 %) | −0.51550 (−2.20 %) |

跟 Ghia 的表的偏差不會隨網格變細而縮小，Re = 1000 時甚至變大。原因在最後兩列：Ghia 的表本身是 129² 網格上的
二階解，它的極值跟譜方法的標準答案差了 1.5–3.3%。這個解算器是往譜方法的值收斂（Re = 1000：2.2% → 0.47% → 0.15%），
所以在兩者不同的地方自然會離開 Ghia 的數字。Botella–Peyret 的值是從別的論文轉引的（見 REFERENCES.md）。

### Schäfer–Turek 圓柱標準考題

2D-1（定常，Re = 20）：

| cells per D | lattice U | c_D | c_L | L_a (m) | ΔP (Pa) |
|---|---|---|---|---|---|
| 10 | 0.04 | ✗ 5.6132 (+0.42 %) | ✓ 0.01050 | ✗ 0.0802 (−4.7 %) | ✗ 0.1133 (−3.4 %) |
| 20 | 0.04 | ✓ 5.5891 | ✓ 0.01052 | ✗ 0.0834 (−1.0 %) | ✗ 0.1159 (−1.1 %) |
| 40 | 0.04 | ✓ 5.5789 | ✓ 0.01079 | ✗ 0.0838 (−0.4 %) | ✗ 0.1170 (−0.17 %) |
| 80 | 0.04 | ✓ 5.5774 | ✓ 0.01079 | ✗ 0.0840 (−0.2 %) | ✓ 0.1173 |
| 20 | 0.02 | ✗ 5.5928 (+0.05 %) | ✗ 0.01037 (−0.3 %) | ✗ 0.0838 (−0.5 %) | ✗ 0.1162 (−0.9 %) |
| 1996 interval | | 5.57 – 5.59 | 0.0104 – 0.0110 | 0.0842 – 0.0852 | 0.1172 – 0.1176 |
| later reference | | 5.57954 | 0.010619 | — | 0.11752 |

2D-2（週期性渦流脫落，Re = 100；ΔP 取升力最大值之後半個週期）：

| cells per D | lattice U | c_D max | c_D min | c_L max | c_L min | St | ΔP (Pa) |
|---|---|---|---|---|---|---|---|
| 10 | 0.05 | diverges | | | | | |
| 20 | 0.05 | ✗ 3.3244 (+2.6 %) | 3.2719 | ✗ 0.9807 (−0.9 %) | −1.0175 | ✓ 0.2988 | ✓ 2.4700 |
| 40 | 0.05 | ✗ 3.2423 (+0.07 %) | 3.1919 | ✗ 0.9830 (−0.7 %) | −1.0181 | ✓ 0.3006 | ✓ 2.4734 |
| 80 | 0.05 | ✓ 3.2354 | 3.1840 | ✓ 0.9947 | −1.0295 | ✓ 0.3007 | ✓ 2.4840 |
| 20 | 0.025 | ✗ 3.3268 (+2.7 %) | 3.2633 | ✗ 0.9797 (−1.0 %) | −1.0167 | ✓ 0.2995 | ✓ 2.4746 |
| 40 | 0.025 | ✗ 3.2431 (+0.10 %) | 3.1790 | ✗ 0.9792 (−1.1 %) | −1.0144 | ✓ 0.3013 | ✓ 2.4769 |
| 80 | 0.025 | ✓ 3.2359 | 3.1702 | ✓ 0.9902 | −1.0251 | ✓ 0.3015 | ✓ 2.4872 |
| 20 | 0.0125 | ✗ 3.3247 (+2.6 %) | 3.2634 | ✗ 0.9781 (−1.2 %) | −1.0151 | ✓ 0.2996 | ✓ 2.4724 |
| 40 | 0.0125 | ✓ 3.2395 | 3.1784 | ✗ 0.9763 (−1.4 %) | −1.0115 | ✓ 0.3015 | ✓ 2.4741 |
| 1996 interval | | 3.22 – 3.24 | — | 0.99 – 1.01 | — | 0.295 – 0.305 | 2.46 – 2.50 |
| later reference | | 3.2274 | 3.1643 | 0.9866 | −1.0213 | 0.3018 | 2.4848 |

這些表說了什麼（包括不好看的部分）：

* **定常題。** 阻力收斂到與後來的譜方法值相差 0.04% 以內，壓力差在每個直徑 80 格時進入區間。
  回流區長度從下方收斂，80 格時仍比區間低 0.2%；它是用節點之間的線性內插量的。
* **週期題的最大值。** 每個直徑 80 格時，四個考題量都在 1996 年的區間內；格子速度 0.05 如此，減半（Ma 0.043）
  之後也一樣。80 格把速度減半，c_Dmax 只變了 +0.015%（3.2354 → 3.2359），不是先前從 20、40 格推估的 +0.1%，
  所以「低馬赫數時會落到區間上緣」的猜測是錯的。c_Lmax 降了 0.45%，變成 0.9902，剛好還在區間內。這個區間並不包含
  後來更精確的參考值（0.9866），所以「在區間內」不等於「正確」：80 格時這個解算器的最大升力比後來的參考值高 0.4%
  （U = 0.025；U = 0.05 時高 0.8%），40 格時則低 0.4–1.0%。80 格、U = 0.025 的 St（0.3015）和 ΔP（2.4872）
  與後來的參考值相差都在 0.1% 以內。
* **週期題的阻力擺動與馬赫數。** 阻力的峰對峰值 c_Dmax − c_Dmin（後來的參考值 0.0631；Ma = U √3）：

| cells per D | U = 0.05 (Ma 0.087) | U = 0.025 (Ma 0.043) | U = 0.0125 (Ma 0.022) |
|---|---|---|---|
| 20 | 0.0525 (−16.8 %) | 0.0634 (+0.4 %) | 0.0613 (−2.9 %) |
| 40 | 0.0504 (−20.1 %) | 0.0642 (+1.6 %) | 0.0611 (−3.2 %) |
| 80 | 0.0514 (−18.6 %) | 0.0657 (+4.1 %) | queued, not yet run |

先前的版本發現 U = 0.05 時振幅小了 18%，並歸因於可壓縮效應。新的計算證實這個不足來自馬赫數、不是解析度：
同一個網格上把速度減半，振幅增加 21–28%；固定速度把網格加密四倍，只變 2–4%。但新的計算並不支持當時附帶的簡單說法，
也就是「誤差與 Ma² 成正比、降低馬赫數就會消失」：速度再減半一次，振幅反而又*下降* 3–5%，兩種網格上都變成比參考值
低 3%。如果誤差與 Ma² 成正比，每減半一次誤差應該維持同號、縮小成四分之一。所以 U = 0.025 時的吻合有一部分是運氣，
即使在 Ma = 0.02，振幅仍帶著幾個百分點的馬赫數不確定性。一個可能的解釋（這裡沒有驗證）：管道長 22 個直徑，
三種速度下聲音走完全長分別需要 1.9、0.95、0.48 個對流時間單位，而阻力擺動的週期是 1.66 個單位；
阻力振幅可能是跟著這兩個時間的比例走，而不只是 Ma²。c_Dmax 和 ΔP 也不是單調變化，但變動分別不超過 0.1% 和 0.2%。
史特豪數倒是像 Ma² 誤差：每減半一次就上升，第二次的變化比第一次小 5–6 倍，外插到 Ma = 0，40 格時是 0.3016，
後來的參考值是 0.3018。報告裡的「Mach-number dependence」表列出了每個量的這些比例。

* **粗網格。** 每個直徑 10 格時週期題會發散（鬆弛時間 0.515）；程式會回報發散，而不是印出數字。

## 台北 101 實驗

台北 101 的平面是正方形，四個角各往內退兩階。結構設計者寫道：風洞試驗中「直角的方塔會產生很大的橫風向激振。
圓角與（45°）切角降低了側向反應，但 2.5 公尺缺口的『鋸齒』或『雙缺口』角達到顯著的降低」
（Poon、Shieh、Joseph 與 Chang 2004，原文為英文）；風洞實驗室 RWDI 的 Irwin（2008）則說，把角修軟
「使風造成的基底彎矩降低約 25%」。

這裡把五種等寬（D）的截面放進同一個風洞（均勻來流、阻塞比 5%、零攻角）：直角、切角（邊長 0.1 D）、
圓角（半徑 0.1 D）、單層內凹（0.1 D 的缺口）、雙層內凹（兩階各 0.05 D）。`make corners` 可重現
（機器空閒、4 執行緒時，主要那組約 36 分鐘，三組敏感度測試共約 30 分鐘；這裡的結果是在忙碌的機器上跑的，花了 2.2 倍時間）。

主要那一組，Re = 200，每邊 60 格（括號內是相對於直角方柱的變化；表中依序為直角、切角、圓角、單層內凹、雙層內凹（鋸齒））：

| section | mean drag C_D | RMS lift C_L' | Strouhal St |
|---|---|---|---|
| sharp square | 1.483 | 0.410 | 0.1594 |
| chamfered | 1.348 (−9.1 %) | 0.366 (−10.8 %) | 0.1817 (+14.0 %) |
| rounded | 1.370 (−7.6 %) | 0.354 (−13.5 %) | 0.1778 (+11.5 %) |
| single recess | 1.364 (−8.1 %) | 0.382 (−6.7 %) | 0.1817 (+14.0 %) |
| double recess (saw-tooth) | 1.352 (−8.9 %) | 0.375 (−8.5 %) | 0.1825 (+14.5 %) |

敏感度測試（每邊 40 格；依序為較粗的網格、馬赫數減半、Re = 100）：

| set | section | C_D | C_L' | St |
|---|---|---|---|---|
| Re = 200, coarser grid | sharp square | 1.465 | 0.390 | 0.1642 |
| | chamfered | 1.350 (−7.9 %) | 0.365 (−6.4 %) | 0.1816 (+10.6 %) |
| | rounded | 1.374 (−6.2 %) | 0.354 (−9.2 %) | 0.1779 (+8.3 %) |
| | single recess | 1.365 (−6.8 %) | 0.385 (−1.3 %) | 0.1820 (+10.8 %) |
| | double recess | 1.353 (−7.6 %) | 0.377 (−3.4 %) | 0.1827 (+11.3 %) |
| Re = 200, half the Mach number | sharp square | 1.451 | 0.385 | 0.1648 |
| | double recess | 1.340 (−7.6 %) | 0.367 (−4.7 %) | 0.1829 (+11.0 %) |
| Re = 100 | sharp square | 1.500 | 0.183 | 0.1491 |
| | chamfered | 1.429 (−4.7 %) | 0.182 (−0.5 %) | 0.1554 (+4.2 %) |
| | rounded | 1.448 (−3.5 %) | 0.178 (−3.0 %) | 0.1534 (+2.9 %) |
| | single recess | 1.432 (−4.5 %) | 0.188 (+2.5 %) | 0.1563 (+4.8 %) |
| | double recess | 1.425 (−5.0 %) | 0.184 (+0.6 %) | 0.1564 (+4.9 %) |

在這個模型裡，每一種修角都讓平均阻力下降（Re = 200 約 8–9%，Re = 100 為 3.5–5%）、渦流脫落頻率上升
（12–15% 與 3–5%），也就是尾流變窄了。這兩點在兩種網格、馬赫數減半之後都成立。升力擺動（RMS）在 Re = 200
的主網格上下降 7–14%，但這是最不確定的數字：它是相對於直角方柱算的，而直角方柱正是對網格最敏感的那個
（兩種網格之間阻力差 1.2%、史特豪數差 3%，修角截面則都在 0.3% 以內），在較粗的網格上降幅只有 1–9%。
Re = 100 時升力擺動的變化不超過 ±3%。四種修角幾乎可以互換；雙層內凹並沒有比同尺寸的切角好。

直角方柱方面，Sohankar、Norberg 與 Davidson（1998）在最細網格（阻塞比 5%）給的是：Re = 100 時 C_D 1.478、
St 0.146、C_L' 0.153；Re = 200 時 C_D 1.462、St 0.150、C_L' 0.377。這裡的阻力在兩個雷諾數都高 1.5%，
史特豪數高 2% 與 6%，升力 RMS 高 20% 與 9%。

**這個實驗能說什麼、不能說什麼。** 模擬是二維、層流、雷諾數 100–200。真實的塔是三維的，處在有紊流和風速梯度的
風場裡，雷諾數約 10⁸；而且實際量測的是結構反應（基底彎矩、加速度），那還取決於漩渦脫落沿高度方向同步的程度，
以及脫落頻率離建築自然頻率多近。

* **定性上重現的：** 文獻指出切角與圓角的方柱阻力較低、尾流較窄（Tamura 與 Miyagi 1999，引自 Dey 與 Das 2016
  的整理），而且史特豪數隨圓角增大而上升（Miran 與 Sohn 2015，Re = 500）。這裡看到同樣的方向。
* **沒有重現、也不該期待重現的：** 設計者觀察到「雙缺口遠比切角、圓角有效」。在這個二維層流模型裡，
  幾種修角截面的表現幾乎一樣。實際尺度下通常被認為起作用的機制（缺口擾亂紊流中的分離剪力層與再附著、
  沿高度方向的相關性下降）在這個模型裡都不存在。
* **完全不能比的：** 25% 的基底彎矩降幅。這裡沒有任何量是基底彎矩，本頁的任何百分比都不該拿去跟它對照。
* 找不到二維層流條件下研究內凹角的已發表論文，所以內凹角的結果沒有直接的對照；直角方柱則在報告中與
  Sohankar、Norberg 與 Davidson（1998）比較。

## 效能

`make bench`：圓柱標準考題的設定（TRT，每一步都處理邊界並計算受力），Apple M5（10 核心、16 GB），
在機器沒有其他工作時量測（欄位：網格、流體格點數、1／2／4 執行緒）：

| grid | fluid nodes | 1 thread | 2 threads | 4 threads |
|---|---|---|---|---|
| 882 × 166 | 143 056 | 130 MLUPS | 243 MLUPS | 374 MLUPS |
| 1762 × 330 | 572 256 | 131 MLUPS | 251 MLUPS | 395 MLUPS |

MLUPS = 每秒更新幾百萬個格點。基準測試印出的狀態雜湊值在每一種執行緒數下都相同。這台機器是共用的：
同一個測試在其他工作同時執行（負載平均超過 8）時量到的是 108、88、133 MLUPS，所以上表應視為最佳情況。

作為量級參考：lbmpy 的論文（Bauer、Köstler 與 Rüde 2021，第 V.A 節）報告，自動產生、以 AVX-512 向量化的 D3Q19
單鬆弛時間核心，在一顆 Xeon Gold 6148 的 20 個核心上「約 300 MLUP/s」，受記憶體頻寬限制；換算每核心 15 MLUPS、
每格點 19 個分佈，約每核心每秒 2.85 億次分佈更新。這裡的純量核心單核心 130 MLUPS、每格點 9 個分佈，
約每秒 11.7 億次分佈更新，4 執行緒時每核心約 8.9 億次。這不是對等的比較（二維對三維、單核心小網格對記憶體飽和的
整顆處理器、相隔多年的不同機器）；它只說明一個直接寫的 pull 式核心落在正式程式的量級內，不代表比較快。

## 測試

`make test` 約半分鐘跑完 68 個測試：

* 單元測試：格子的等向性、平衡分佈的矩、碰撞不變量（BGK、TRT、含外力）、FFT 對照直接計算的 DFT、
  峰值與頻率估計、單位換算、幾何、PNG/DEFLATE 與 GIF/LZW 的編解碼來回與損壞處理（截斷的檔案、亂數輸入、
  解壓縮炸彈）、色階順序、圖表；
* 解算器測試（`tests/solver.rs`）：週期盒內質量與動量守恆到捨入誤差、外力給的衝量完全正確、封閉盒質量守恆、
  從四個移動座標系看 Couette 流（牆面剪應力等於 ρν dU/dy，誤差 1e-12）、隨流移動的物體受力為零、
  均勻流通過開放邊界不變、入口與出口驅動的完全發展管流、不同執行緒數結果逐位元相同、出口吸收聲波；
* 編解碼測試（`tests/codecs.rs`）：PNG 讀取對照 Python zlib 寫出的檔案（獨立的編碼器）；
* 回歸測試（`tests/validation.rs`）：每個驗證題在小網格上、以固定容許誤差對照論文數字，另加一個縮小版的修角實驗。

CI（`.github/workflows/ci.yml`）在 Ubuntu 上跑格式檢查、clippy、全部測試、快速驗證組、示範，以及一次自訂形狀。

## 限制

* 二維、層流、均勻網格。沒有紊流模型、沒有局部加密、物體不能移動。
* 弱可壓縮：誤差是 Ma² 量級；當計算區域的長度與「聲音在一個擺動週期內走的距離」相當時，
  非定常受力會明顯受影響。2D-2 的阻力振幅在降低馬赫數時甚至不是單調收斂（見 2D-2 的討論）。
* 開放邊界只有一階精度。出口只吸收平面波；風洞側邊界對平均流有些微穿透（Ma 量級）。
* 內插反彈不完全守恆質量；在只有一格寬的凹角會退回一般反彈，所以比大約三格還細的特徵表現不好。
* 直角方柱的結果對網格敏感（見報告裡的粗網格組）。
* PNG 輸入不能是交錯式（interlaced）；PNG 輸出只用固定霍夫曼碼。
* 效能數字是在與其他工作共用的機器上量的。
* 各方法論文的公式是照教科書上的標準形式實作、再用這裡的測試驗證，沒有逐行對照原始論文；
  哪些文獻實際讀過，逐條寫在 docs/REFERENCES.md。

## 相關作品

正式的格子波茲曼程式：**Palabos**（Latt 等人 2021）、**OpenLB**（Krause 等人 2021）、**waLBerla**
（Bauer 等人 2021）、**Sailfish**（Januszewski 與 Kostur 2014，GPU）、**lbmpy**（Bauer、Köstler 與 Rüde 2021，
程式碼產生）。它們有三維、多種碰撞模型、網格加密、紊流模型、大規模平行，驗證也遠比這個專案完整。
vortexlab 只是一個小型的教學用解算器，以上功能都沒有，也沒有使用它們的任何程式碼。做它的目的是把每一層
（碰撞、邊界、受力、頻譜分析、影像編解碼、報告）都自己做一遍來學這個方法，並練習誠實地拿文獻來檢驗數值工具。
完整引用見 [docs/REFERENCES.md](docs/REFERENCES.md)。

## 建置

需要新版的 stable Rust（以 1.98 建置與測試）。其他都不用：沒有相依套件、不連網路、不開連接埠。

```
make build      # cargo build --release
make test       # all tests
make lint       # rustfmt + clippy
make demo       # coarse benchmark + animated vortex street
make quick      # every validation case on small grids (about a minute)
make validate   # full validation
make corners    # corner experiment
make bench      # MLUPS
make report     # regenerate docs/report from results/
```

## 授權

MIT，見 [LICENSE](LICENSE)。
