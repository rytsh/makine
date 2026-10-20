<!--
seviyeler: FastAdd32, PipeAdd32, Forward, Cache4
-->
# Hızlı işlemci: Apple ve ARM çekirdekleri neden hızlı?

İşlemci seviyesinde kurduğun RISC-V tek döngülüdür: her saat vuruşunda
bir komut baştan sona çalışır. Bu en anlaşılır tasarımdır, ama en yavaşıdır.
Bu bölüm gerçek işlemcileri hızlı yapan fikirleri kapı kapı gösterir.

## Saat ne kadar hızlı vurabilir?

Her NAND kapısının çıkışı, girişi değiştikten kısa bir süre sonra değişir.
Bir sinyal kapıdan kapıya geçerken bu gecikmeler toplanır. Saat ancak
**en uzun yol** (kritik yol) durulduktan sonra vurabilir. Yoksa yazmaçlar
yarım kalmış bir sonucu saklar.

makine bu yolu ölçer: çözüm panelindeki "en uzun yol" sayısı, bir sinyalin
geçmek zorunda kaldığı en fazla NAND sayısıdır. Örnek çözümlerde:

| Devre | En uzun yol (NAND) |
|---|---|
| Adder32 (zincirleme elde) | 68 |
| FastAdd32 (elde seçmeli) | 28 |
| RISC-V işlemcinin tamamı | 2411 |

İşlemcinin yolu bu kadar uzun, çünkü bölme birimi 32 çıkarmayı art arda
yapar. Bölme olmasa yol 122 kapıya iner. Gerçek işlemciler bu yüzden bölmeyi
tek bir saatte değil, birçok saatte adım adım yapar.

## 1. Daha kısa yol: hızlı toplayıcı

Zincirleme toplayıcıda elde, en düşük bitten en yükseğe sırayla yürür. 32 bit
için bu 32 tam toplayıcı demektir. **Elde seçmeli** toplayıcı, üst baytları iki
kez toplar: bir kez elde 0 varsayarak, bir kez elde 1 varsayarak. Gerçek elde
gelince bir MUX doğru sonucu seçer. Kapı sayısı iki katına çıkar ama yol yarıya
iner. Bu bir **alan–hız takasıdır**; çip tasarımı bu takaslarla doludur.

Gerçek işlemciler bunun daha gelişmiş halini, **elde öngörülü**
(carry-lookahead) toplayıcıyı kullanır. Orada elde, ağaç şeklinde bir devreyle
log₂(32) = 5 adımda hesaplanır.

## 2. Boru hattı (pipeline)

Bir çamaşırhane düşün: yıkama, kurutma, ütüleme. İlk çamaşır kurutmaya
geçince makine boş kalmaz, ikinci çamaşır yıkanmaya başlar. Tek bir çamaşırın
işi aynı sürer, ama her aşama hep doludur.

İşlemcilerde aşamalar genellikle beştir:

| Aşama | Ne yapar |
|---|---|
| IF (getir) | pc'deki komutu bellekten okur |
| ID (çöz) | komutu çözer, register'ları okur |
| EX (yürüt) | ALU hesaplar |
| MEM (bellek) | yükleme ya da saklama yapar |
| WB (geri yaz) | sonucu register'a yazar |

Aşamaların arasına yazmaçlar konur. Her saat vuruşunda her komut bir sonraki
aşamaya geçer. Aşamalar kısa olduğu için saat beş kata kadar hızlanabilir.
**PipeAdd32** seviyesinde bunu bir toplayıcıyla yaparsın: iş ikiye bölünür,
araya 49 bitlik bir yazmaç girer, en uzun yol yarıya iner.

İki kavramı ayırmak önemli:

- **Gecikme (latency):** bir sonucun çıkması için geçen saat sayısı. Boru
  hattı bunu artırır.
- **Verim (throughput):** saat başına çıkan sonuç sayısı. Boru hattı bunu
  korur, saat hızlandığı için de saniyede çok daha fazla iş çıkar.

## 3. Tehlikeler ve veri yönlendirme

Boru hattı yeni bir sorun getirir. Şu iki komutu düşün:

```
add x5, x1, x2
sub x6, x5, x3
```

`sub`, x5'i ID aşamasında okur. Ama `add` o sırada daha EX aşamasındadır;
sonucu register dosyasına iki saat sonra yazılacaktır. `sub` eski değeri
okursa yanlış hesaplar. Buna **veri tehlikesi** (data hazard) denir.

Çözüm, `add`'in sonucunu beklemeden doğrudan `sub`'ın ALU girişine
**yönlendirmektir** (forwarding). **Forward** seviyesinde bu kararı veren
devreyi kurarsın: hangi aşamadaki sonuç, hangi girişe gidecek?

Her tehlike yönlendirmeyle çözülmez. Bir yükleme komutunun (`lw`) verisi
ancak MEM aşamasında gelir. Ondan hemen sonraki komut bu veriyi isterse,
işlemci bir saat **durmak** zorundadır (stall). Dallanmalarda da benzer bir
sorun vardır: hangi komutun getirileceği, dallanma yürütülene kadar belli
değildir. Bu yüzden gerçek işlemciler **dallanma tahmini** yapar.

## 4. Önbellek

Bir işlemcinin bir toplama yapması yaklaşık bir nanosaniye sürer. Ana
bellekten (DRAM) bir kelime okumak ise yaklaşık 100 nanosaniye. İşlemci her
erişimde belleği bekleseydi zamanının neredeyse tamamı beklemekle geçerdi.

**Önbellek**, son kullanılan verilerin işlemcinin yanında tutulan küçük ve
hızlı bir kopyasıdır. Programlar aynı verilere tekrar tekrar ve yan yana
erişme eğilimindedir. Bu yüzden küçük bir önbellek bile erişimlerin çoğunu
yakalar.

**Cache4** seviyesinde 4 satırlık bir önbellek kurarsın:

- Adresin alt bitleri **satırı** seçer.
- Üst bitleri **etiket** olarak saklanır. Satırdaki verinin hangi adrese ait
  olduğunu etiket söyler.
- **Geçerli** biti, açılışta boş bir satırın yanlışlıkla eşleşmesini önler.

Gerçek işlemcilerde önbellekler katmanlıdır: L1 (birkaç on KB, birkaç saat),
L2 (birkaç MB), L3 (on MB'larca). Apple M1'in performans çekirdeklerinde
L1 komut önbelleği 192 KB'tır.

## 5. Ötesi: Apple ve ARM çekirdekleri

Bu bölümdeki dört fikir, modern çekirdeklerin temelidir. Üstüne şunlar gelir:

- **Süperskaler:** her saatte birden fazla komut. M1'in performans çekirdeği
  8 komutu birden çözer.
- **Sırasız yürütme (out-of-order):** hazır olan komut, sırası gelmese de
  çalışır. Yüzlerce komut aynı anda "uçuşta" olabilir.
- **Dallanma tahmini:** işlemci, dallanmanın sonucunu geçmişe bakarak tahmin
  eder ve %95'ten fazla isabet ettirir.
- **Daha derin boru hatları** ve daha çok önbellek.

Bunların her biri, burada kurduğun parçaların büyütülmüş ve birbirine
bağlanmış halidir: yazmaçlar, MUX'lar, karşılaştırıcılar ve toplayıcılar.

İşlemcini gerçek bir FPGA'da çalıştırmak için [Gerçek çip](silikon.md)
sayfasına bak.
