<!--
seviyeler:
-->
# Gerçek çip: makine'den silikona

makine'de kurduğun her çip gerçek donanım olabilir. Bunun için çipin önce
**Verilog**'a çevrilmesi gerekir. Verilog, çip endüstrisinin devre tarif
ettiği dildir. makine bu çeviriyi senin yerine yapar.

## Bir çip nasıl tasarlanır?

Gerçek bir çip kabaca altı adımda tasarlanır:

1. **Mimari:** Çipin ne yapacağına karar verilir: komut seti, bit genişliği,
   bellek yapısı.
2. **RTL:** Devre Verilog ya da VHDL gibi bir dille yazılır. makine'de bu
   adımın karşılığı, çipi kapılarla ve çiplerle kurmaktır.
3. **Doğrulama:** Devre test vektörleriyle simüle edilir. makine'deki
   **Test et** düğmesi bu işi yapar.
4. **Sentez:** Devre, dökümhanenin kütüphanesindeki kapılara (hücrelere)
   çevrilir. Bu adım Yosys gibi araçlarla yapılır.
5. **Yerleşim ve yönlendirme:** Hücreler silikonun üzerine yerleştirilir ve
   aralarındaki teller çizilir. Ardından zamanlama analiz edilir (OpenROAD).
6. **Üretim (tape-out):** Ortaya çıkan GDS dosyası dökümhaneye gider.

İlk üç adım makine'de yapılır. Geri kalanı için makine sana gereken
dosyaları hazırlar.

## Verilog projesi

Bir seviyenin ya da serbest çipin panelinde **🔌 Gerçek donanıma aktar →
Verilog projesi kaydet** düğmesine bas. Seçtiğin klasöre şu dosyalar yazılır:

| Dosya | İçinde ne var |
|---|---|
| `design.v` | Çipin ve kullandığı bütün çipler. Her çip ayrı bir modüldür. |
| `tb.v` | Test tezgâhı. Seviyenin kendi testi ya da makine'nin simülasyonu. |
| `synth.ys` | Yosys betiği: tasarımı NAND kapılarına sentezler. |
| `Makefile` | `make sim`, `make wave`, `make synth` |

Araçlar ücretsizdir: `apt install iverilog yosys gtkwave` ya da
[OSS CAD Suite](https://github.com/YosysHQ/oss-cad-suite-build). Klasörde
`make sim` çalıştırdığında `PASS` yazısını görürsen Verilog, makine'deki
devrenle birebir aynı davranıyor demektir.

Verilog'da hiçbir şey sadeleştirilmez. Her NAND kapın `~(a & b)` olarak
yazılır ve çiplerin, makine'deki gibi iç içe modüller halinde kalır.

## Hafıza: mandallar ve flip-flop'lar

NAND kapılarından kurulan bir mandal **geri beslemeli bir döngüdür**.
Simülatörler bu döngülerle çalışabilir, ama FPGA ve çip araçları
çalışamaz. Bu araçlar hafızanın hazır flip-flop hücreleriyle yapılmasını
bekler.

Bu yüzden makine, aktarırken `Latch`, `DLatch` ve `DFF` çiplerini kapı
döngüsü olarak değil, gerçek mandal ve flip-flop olarak yazar. Bunların
dışındaki her şey senin kapıların olarak kalır. Senin çipin de aynı testi
geçtiği için çevresindeki devre aynı şekilde davranır.

RAM, ROM ve ekran gibi **cihazlar** Rust'la yazılmıştır, kapılardan
oluşmazlar. Bu yüzden Verilog'a çevrilemezler. Belleği istersen yazmaçlardan
kendin kurabilirsin; `Ram8` seviyesi bunun nasıl yapıldığını gösterir.

## FPGA'da çalıştırmak

FPGA, içindeki devreyi yeniden programlayabildiğin bir çiptir. Tasarımını
FPGA'da denemek, gerçek bir çip ürettirmekten çok daha ucuz ve hızlıdır.
iCE40 tabanlı kartlar (iCEBreaker, Tang Nano gibi) açık kaynak araçlarla
programlanabilir:

```sh
yosys -p "synth_ice40 -top mk_MiniCpu -json mini.json" design.v
nextpnr-ice40 --up5k --json mini.json --pcf kart.pcf --asc mini.asc
icepack mini.asc mini.bin && iceprog mini.bin
```

Hangi FPGA pininin çipinin hangi ucuna bağlandığını kartının `.pcf`
dosyasına yazarsın.

Boyutlar konusunda fikir vermesi için örnek çözümlerin iCE40'taki hali:
mini işlemci (`MiniCpu`) **70 LUT ve 12 flip-flop** kullanır, her karta sığar.
RISC-V işlemci (`Cpu`) ise **yaklaşık 8800 LUT ve 1024 flip-flop** kullanır.
Flip-flop'ların hepsi register dosyasındadır (32 × 32 bit). Bu yüzden
iCE40 UP5K'ya (5280 LUT) sığmaz. Onun için ECP5 gibi daha büyük bir FPGA
gerekir (`synth_ecp5` ve `nextpnr-ecp5`).

## FPGA bilgisayarı: RISC-V'ni gerçekten çalıştır

İşlemci seviyesini (`Cpu`) geçtiysen, panelde **🖥 FPGA bilgisayarı kaydet**
düğmesi çıkar. Bu düğme işlemcinin etrafına küçük bir bilgisayar kurar.
Elektronikte buna **SoC** (System on Chip, tek çipte sistem) denir:

- **64 KB RAM:** FPGA'nın blok belleği. Program içine yüklenmiş halde gelir.
- **Seri port (UART, 115200 baud):** `printf` ile yazdığın her şey kartın
  USB kablosundan bilgisayarına gelir.
- **Zamanlayıcı:** makine'deki gibi milisaniye sayar.
- **Sıralayıcı:** işlemciyi adım adım sürer.

Kart olarak **ULX3S** kullanılıyor (Lattice ECP5-85F, açık kaynak araçlarla
tamamen programlanabilir). Klasörde üç komut var:

```sh
make sim     # önce bilgisayarda dene (Verilator): programın çıktısı ekrana gelir
make         # FPGA dosyasını (makine.bit) üret, birkaç dakika sürer
make prog    # karta yükle
```

C programını FPGA için derlerken RAM boyutunu söylemen gerekir:

```sh
MK_RAM=64K sdk/build.sh merhaba.c
```

**Neden sıralayıcı var?** makine'deki bilgisayarda bellek anında cevap
verir. FPGA'nın blok belleği ise bir saat vuruşu sonra cevap verir. Ayrıca
işlemcin NAND kapılarından yapıldığı için en uzun yolu (bölücü) yaklaşık
250 ns sürer. Sıralayıcı her komutta önce komutu okur, kapıların durulmasını
bekler, sonra veriyi okur, tekrar bekler ve en sonunda işlemcinin saatini
bir kez vurur. 12,5 MHz'de bu, saniyede yaklaşık 650 bin komut eder.
Tasarımın, makine'nin kendi örnek işlemcisiyle FPGA'ya yerleştirilip
zamanlama analizinden geçtiği doğrulandı. Kartın %13'ünü kullanıyor.

Bunu hızlandırmanın yolu, gerçek işlemcilerin yaptığını yapmaktır: işi
aşamalara bölmek (boru hattı) ve belleği önbellekle hızlandırmak. Bunlar
[Hızlı işlemci](hizli.md) sayfasında.

## Tiny Tapeout: kendi çipin

[Tiny Tapeout](https://tinytapeout.com), birçok kişinin küçük tasarımlarını
tek bir çipte birleştirip ürettirir. Böylece gerçek bir çip birkaç yüz dolara
iner. Her tasarıma yaklaşık 167×108 µm'lik bir alan, 8 giriş, 8 çıkış ve
8 iki yönlü uç düşer. Ayrıca bir saat (`clk`) ve bir reset ucu vardır.

**Tiny Tapeout projesi kaydet** düğmesi şunları hazırlar:

- `info.yaml`: projenin bilgileri ve hangi pinin ne işe yaradığı.
- `src/project.v`: Tiny Tapeout'un istediği `tt_um_...` modülü. Senin
  çipini pinlere bağlar.
- `src/makine.v`: çipinin kendisi.
- `test/`: pinlerin üzerinden çalışan test.

Pinler şöyle dağıtılır:

- `clk` adlı giriş, Tiny Tapeout'un saatine bağlanır.
- `reset` adlı giriş, reset ucuna bağlanır. Tiny Tapeout'ta reset ters
  çalışır (0 iken reset), makine bunu senin için çevirir.
- Diğer girişler sırayla `ui_in`'e, sığmayanlar `uio`'ya gider.
- Çıkışlar sırayla `uo_out`'a, sığmayanlar `uio`'ya gider.

Toplam 24 uç vardır. Bir yazmaç, sayaç ya da 8 bitlik bir ALU sığar
(örneğin `MiniAcc8`). Mini işlemci (`MiniCpu`) ise 16 giriş ve 17 çıkış biti
ister, yani olduğu gibi sığmaz. Büyük bir tasarımı sığdırmanın yolu,
verileri sırayla göndermektir: bir serbest çip yap, içine işlemciyi koy ve
çıkışları bir seçici girişle sırayla dışarı ver. Program ROM'unu da çipin
içine kapılarla koyarsan `instr` girişine gerek kalmaz.

Göndermek için
[ttsky-verilog-template](https://github.com/TinyTapeout/ttsky-verilog-template)
şablonundan bir GitHub deposu oluştur. Klasördeki `info.yaml`, `src/` ve
`test/` dosyalarını içine kopyala ve depoyu gönder. GitHub Actions tasarımı
sentezler, yerleştirir ve çipin resmini çizer. Hepsi yeşil olursa bir sonraki
üretim turuna (shuttle) başvurabilirsin.

Alan konusunda bir uyarı: bir Tiny Tapeout karesine birkaç bin kapı sığar.
Tasarımın büyükse `info.yaml`'daki `tiles` değerini artırman gerekir.
`make synth` kaç hücre kullandığını gösterir.
