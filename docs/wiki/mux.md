<!--
seviyeler: Mux, Demux, Mux32
-->
# Seçici (MUX) ve bus'lar

## MUX: "hangisi gitsin?"

Bir **seçici** (multiplexer, MUX) iki girişten birini çıkışa verir.
Hangisinin geçeceğine **sel** (select, seçim) girişi karar verir:

| sel | çıkış |
|---|---|
| 0 | a |
| 1 | b |

Bilgisayarın her yerinde MUX vardır. Örneğin:

- ALU'ya register mı gitsin, sabit sayı mı?
- Register'a toplama sonucu mu yazılsın, bellekten okunan mı?
- Sıradaki komut `pc + 4`'ten mi gelsin, bir atlama hedefinden mi?

### Nasıl yapılır?

```
out = (a VE DEĞİL sel)  VEYA  (b VE sel)
```

`sel = 0` iken sağdaki VE 0 olur, soldaki ise a'yı geçirir. `sel = 1`
iken tam tersi. Bir NOT, iki AND ve bir OR yeter.

## DEMUX: tersi

**Dağıtıcı** (demultiplexer) bir girişi iki çıkıştan birine gönderir:

| sel | a | b |
|---|---|---|
| 0 | giriş | 0 |
| 1 | 0 | giriş |

Register dosyasında "hangi register'a yazılacak?" sorusunu DEMUX'lardan
yapılmış bir ağaç cevaplar.

## Bus'lar: birçok tel bir arada

Bir sayı 32 bittir; 32 ayrı tel çekmek eziyet olurdu. Oyunda bir **bus**
(ör. `a[32]`), tek bir kalın mavi tel olarak çizilen 32 teldir.

### Tekrar (×N)

Bir parçaya sağ tıklayıp **Tekrar** sayısını 32 yaparsan o parçanın 32
kopyası yan yana çalışır. Her ucu da 32 bit genişler. 32 bitlik bir MUX
böylece **tek parça** ile kurulur: `Mux ×32`.

`sel` gibi 1 bitlik bir teli geniş bir girişe bağlarsan, o bit **bütün
kopyalara** dağıtılır. Yani 32 MUX'un hepsi aynı `sel`'i görür. Tam
istediğimiz şey.

### Böl ve Birleştir

- **Böl** bir bus'ı parçalara ayırır: `Böl 8,8,8,8`, 32 biti dört bayta
  ayırır. Bit 0 en düşük basamaktır.
- **Birleştir** tersini yapar.

Bunlar kapı değildir; sadece telleri yeniden düzenlerler, devrede hiçbir
maliyetleri yoktur. Gerçek çiplerde de "teli başka yere götürmek"
bedavadır.

> [!TIP]
> Sabit bir miktar kaydırmak (ör. "1 sola kaydır") da sadece tel işidir:
> `Böl 31,1`, sonra `Birleştir 1,31`'in en alt bitine bir **Sabit 0**.
> [ALU](alu.md) sayfasındaki kaydırıcı tam bu fikri kullanır.

Sırada: [Toplama](toplama.md).
