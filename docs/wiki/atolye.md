<!--
seviyeler: Mux4, Adder4, Register4, Counter12
-->
# İşlemci atölyesi: küçük bus'larla büyük fikirler

RISC-V yolunda 32-bit parçalar kurdun. Bir 4-bit işlemcide aynı fikirleri
daha küçük sayılarla görebilirsin. Bu bölümün dört alıştırması,
[4004 atölyesinin](4004.md) temel parçalarını ayrı ayrı düşünmek içindir.
Bu seviyeleri geçmek örnek projeyi açmak için zorunlu değildir.

## 4-bit seçici

Bir MUX, “hesaplanan değer mi, eski değer mi?” sorusunun devresidir.
`sel=0` iken `a`, `sel=1` iken `b` çıkar. Dört MUX'u paralel koyarsan
4-bit bir bus seçersin; hepsi aynı `sel` bitini okur.

**Dene:** a=3, b=12 olsun. sel değişince çıkış 3 ile 12 arasında değişmeli.
Bitleri ters sırada bağlarsan 3 yerine 12 görmen tesadüf değildir:
`0011`'in ters sırası `1100` olur.

## 4-bit toplayıcı ve taşıma

Tam toplayıcı üç bit alır: a, b ve önceki basamaktan gelen taşıma.
Dört tanesini sıraya bağlarsan düşük bitten yüksek bite taşıma ilerler.

```
bit 0 → taşıma → bit 1 → taşıma → bit 2 → taşıma → bit 3 → carry
```

`15 + 1 = 16`, dört bitlik `sum=0` ve `carry=1` demektir. Carry'yi atarsan
sonucun bir parçasını atmış olursun. `15+15+1` için `sum=15, carry=1` beklenir.

## 4-bit yazmaç

Bir toplayıcının çıkışı giriş değişince değişir; yazmaç ise son yazılan
değeri saklar. `load=1` **ve yükselen saat kenarı** gerekir. Saat 1'de
dururken d değişse bile tekrar yazmamalıdır.

Bu, yalnızca “clk=1 iken geçir” diyen bir D mandaldan farklıdır. Yanlış
parçayı kullanırsan aynı saat darbesinde devre birden fazla işlem yapabilir.

## 12-bit program sayacı

12 bit, 0–4095 adreslerini seçer. Öncelik:

1. reset=1: sıfırla.
2. değilse load=1: d adresine git.
3. değilse inc=1: bir artır.
4. aksi halde eski adresi koru.

`4095 + 1` mod 4096 hesabıyla 0'dır. Buradaki sayaç **bir** artırır;
RISC-V yolundaki 32-bit sayaç sabit uzunluklu komutlar için dört artırıyordu.
4004'te komutlar bir veya iki bayt olabildiğinden kontrol birimi uygun
sonraki adresi seçer.

## Bunlar birleşince

Yazmaç mevcut durumu saklar; toplayıcı ve seçiciler bir sonraki durumu
hesaplar; program sayacı okunacak komutu seçer. İşlemciyi birbiriyle
konuşan bu küçük devreler olarak okumak, onu tek bir sihirli kutu olarak
görmekten daha faydalıdır.

Devam: [Modüllerle düzenlemek](moduller.md), [Sinyal izlemek](sinyaller.md).
