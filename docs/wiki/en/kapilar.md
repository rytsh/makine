<!--
seviyeler: And, Or, Xor
-->
# Logic gates

Logic gates are circuits that make simple decisions about bits. Each gate has
a **symbol** and a **truth table**. The truth table shows the output for every
possible input.

![Basic logic gates and their truth tables](img/kapilar.png)

In the game you see these gates on the canvas with the same symbols. If you
prefer the box view, you can turn off the **Gate symbols** checkbox on the left.

## Making all of them from NAND

![NOT, AND, and OR from NAND](img/nand-her-sey.png)

### NOT
`NAND(a, a)`. The same signal goes to both inputs. Details on the
[NAND](nand.md) page.

### AND
NAND means "NOT AND." So AND = **NOT(NAND(a, b))**: one NAND with a NOT after
it.

### OR: De Morgan's law
Here's a nice trick. The English mathematician **Augustus De Morgan**
(1806–1871) noticed this:

```
a OR b  =  NOT( NOT a  AND  NOT b )
```

In everyday words: saying "a or b is true" is the same as saying "they are not
both false." And `NOT(… AND …)` is exactly NAND:

```
a OR b  =  NAND( NOT a, NOT b )
```

So two NOTs and one NAND are enough.

### XOR (exclusive OR)
XOR gives 1 when the inputs are **different**. It's the heart of addition: the
low digit of 1 + 1 is 0, and the low digit of 1 + 0 is 1. That's exactly XOR!

The classic solution uses 4 NANDs:

```
n   = NAND(a, b)
out = NAND( NAND(a, n), NAND(b, n) )
```

Check why it works with a table:

| a | b | n | NAND(a,n) | NAND(b,n) | out |
|---|---|---|---|---|---|
| 0 | 0 | 1 | 1 | 1 | 0 |
| 0 | 1 | 1 | 1 | 0 | 1 |
| 1 | 0 | 1 | 0 | 1 | 1 |
| 1 | 1 | 0 | 1 | 1 | 0 |

> [!NOTE]
> You can also use chips you made earlier: `(a OR b) AND NOT(a AND b)` is
> XOR too. It's easier to understand but uses more gates. The game accepts
> both.

## Boolean algebra

The math of thinking with gates is called **Boolean algebra**. It's named
after **George Boole** (1815–1864). A few handy rules:

| Rule | Example |
|---|---|
| Double negation | `NOT NOT a = a` |
| De Morgan | `NOT(a AND b) = NOT a OR NOT b` |
| | `NOT(a OR b) = NOT a AND NOT b` |
| Zero/one | `a AND 0 = 0`, `a OR 1 = 1` |
| With itself | `a XOR a = 0`, `a XOR 0 = a` |

In 1937, **Claude Shannon** showed that this algebra matches electrical
circuits one to one. Digital electronics was born that day.

Next: [Selectors and buses](mux.md).
