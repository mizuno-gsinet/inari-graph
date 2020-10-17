# Examples

🐌: Takes a long time to finish graphing.

## Basic Examples

```text
"sin(x) == cos(y)"
```

```text
"y - x == sin(exp(x + y))"
```

```text
"(x^2 + y^2) == 1 || y == -cos(x)"
```

## Examples from [GrafEq](http://www.peda.com/grafeq/)

- 📂 Single Relation/Abstract/Simple/

  - 📄 Parabolic Waves.gqs

    ```text
    "abs(sin(sqrt(x^2 + y^2))) == abs(cos(x))"
    ```

  - 📄 Pythagorean Pairs.gqs 🐌

    ```text
    "floor(x)^2 + floor(y)^2 == floor(sqrt(floor(x)^2 + floor(y)^2))^2" \
    -b -40 40 -40 40
    ```

  - 📄 Pythagorean Triples.gqs 🐌

    ```text
    "floor(x)^2 + floor(y)^2 == 25"
    ```

- 📂 Single Relation/Abstract/Traditionally Difficult/

  - 📄 Infinite Frequency.gqs

    ```text
    "y == sin(40/x)"
    ```

  - 📄 O Spike.gqs

    ```text
    "(x*(x - 3)/(x - 3.001))^2 + (y*(y - 3)/(y - 3.001))^2 == 81"
    ```

  - 📄 Solid Disc.gqs

    ```text
    "81 - x^2 - y^2 == abs(81 - x^2 - y^2)"
    ```

  - 📄 Spike.gqs

    ```text
    "y == x*(x - 3)/(x - 3.001)"
    ```

  - 📄 Step.gqs

    ```text
    "y == atan(9^9^9 * (x - 1))"
    ```

  - 📄 Upper Triangle.gqs

    ```text
    "x + y == abs(x + y)"
    ```

  - 📄 Wave.gqs

    ```text
    "y == sin(x)/x"
    ```

- 📂 Single Relation/Enumerations/Binary/

  - 📄 binary naturals.gqs

    ```text
    "(1 + 99 * floor(mod(floor(y) * exp2(ceil(x)), 2))) * (mod(x, 1) - 1/2)^2 + (mod(y, 1) - 1/2)^2 == 0.15 && \
     floor(-log2(y)) < x && x < 0" -b -15 5 -5 15
    ```

  - 📄 binary squares.gqs

    ```text
    "(1 + 99 * floor(mod(floor(y)^2 * exp2(ceil(x)), 2))) *  (mod(x, 1) - 1/2)^2 + (mod(y, 1) - 1/2)^2 == 0.15 && \
     x < 0 && 0 < floor(y)^2 && floor(y)^2 >= exp2(-ceil(x))" -b -15 5 -5 15
    ```

- 📂 Single Relation/Enumerations/Decimal/

  - 📄 decimal squares.gqs

    ```text
    "(mod(892 * exp2(-floor(mod(floor(y)^2/exp10(-ceil(1.25*x)), 10))), 2) >= 1 && \
      30 * max(abs(mod(y,1) - 1/2), abs(mod(x,0.8)+0.1 - 1/2) + abs(mod(y,1) - 1/2) - 1/4) < 1 || \
      mod(365 * exp2(-floor(mod(floor(y)^2/exp10(-ceil(1.25*x)), 10))), 2) >= 1 && \
      30 * max(abs(mod(y,1) - 1/10), abs(mod(x,0.8)+0.1 - 1/2) + abs(mod(y,1) - 1/10) - 1/4) < 1 || \
      mod(941 * exp2(-floor(mod(floor(y)^2/exp10(-ceil(1.25*x)), 10))), 2) >= 1 && \
      30 * max(abs(mod(y,1) - 9/10), abs(mod(x,0.8)+0.1 - 1/2) + abs(mod(y,1) - 9/10) - 1/4) < 1 || \
      mod(927 * exp2(-floor(mod(floor(y)^2/exp10(-ceil(1.25*x)), 10))), 2) >= 1 && \
      30 * max(abs(mod(x,0.8)+0.1 - 4/5), abs(mod(y,1) - 7/10) + abs(mod(x,0.8)+0.1 - 4/5) - 1/8) < 1 || \
      mod(881 * exp2(-floor(mod(floor(y)^2/exp10(-ceil(1.25*x)), 10))), 2) >= 1 && \
      30 * max(abs(mod(x,0.8)+0.1 - 1/5), abs(mod(y,1) - 7/10) + abs(mod(x,0.8)+0.1 - 1/5) - 1/8) < 1 || \
      mod(325 * exp2(-floor(mod(floor(y)^2/exp10(-ceil(1.25*x)), 10))), 2) >= 1 && \
      30 * max(abs(mod(x,0.8)+0.1 - 1/5), abs(mod(y,1) - 3/10) + abs(mod(x,0.8)+0.1 - 1/5) - 1/8) < 1 || \
      mod(1019 * exp2(-floor(mod(floor(y)^2/exp10(-ceil(1.25*x)), 10))), 2) >= 1 && \
      30 * max(abs(mod(x,0.8)+0.1 - 4/5), abs(mod(y,1) - 3/10) + abs(mod(x,0.8)+0.1 - 4/5) - 1/8) < 1) && \
     x < 0 && 0 < floor(y)^2 && floor(y)^2 >= exp10(-ceil(1.25*x))" -b -7 3 1 11
    ```

- 📂 Single Relation/Enumerations/Trees/

  - 📄 bi-infinite binary tree.gqs

    ```text
    "sin(exp2(floor(y))*x + pi/4*(y - floor(y)) - pi/2) == 0 || \
     sin(exp2(floor(y))*x - pi/4*(y - floor(y)) - pi/2) == 0"
    ```

- 📂 Single Relation/Enumerations/Half-Toned/

  - 📄 Simply Spherical.gqs

    ```text
    "sin(20*x) - cos(20*y) + 2 > 4 * (3/4 - 1/15 * sqrt((x+4)^2 + (y-3)^2)) && (x+1)^2 + (y-1)^2 < 25 || \
     sin(20*x) - cos(20*y) + 2 > 4 * (0.65 + 1/pi * atan(6 * (sqrt((x-1)^2/30 + (y+1)^2/9) - 1))) && (x + 1)^2 + (y - 1)^2 > 25"
    ```

  - 📄 Tube.gqs

    ```text
    "cos(5*x) + cos(5/2 * (x - sqrt(3)*y)) + cos(5/2 * (x + sqrt(3)*y)) > 1 + 3/2 * sin(1/4 * sqrt((x+3)^2 + 2*(y-3)^2)) && \
     (x^2 + 2*y^2 - 1600) * (x^2 + 3*(y-2)^2 - 700) <= 0 || \
     cos(5*x) + cos(5/2 * (x - sqrt(3)*y)) + cos(5/2 * (x + sqrt(3)*y)) > 1 + 2 * atan(1/8 * sqrt(4*(x-2)^2 + 10*(y+4)^2) - 9)^2 && \
     (x^2 + 2*y^2 - 1600) * (x^2 + 3*(y-2)^2 - 700) > 0" -b -50 50 -50 50
    ```

- 📂 Single Relation/Linelike/

  - 📄 Frontispiece #2.gqs

    ```text
    "x/cos(x) + y/cos(y) == x*y/cos(x*y) || x/cos(x) + y/cos(y) == -(x*y/cos(x*y)) || \
     x/cos(x) - y/cos(y) == x*y/cos(x*y) || x/cos(x) - y/cos(y) == -(x*y/cos(x*y))"
    ```

  - 📄 Frontispiece.gqs

    ```text
    "x/sin(x) + y/sin(y) == x*y/sin(x*y) || x/sin(x) + y/sin(y) == -(x*y/sin(x*y)) || \
     x/sin(x) - y/sin(y) == x*y/sin(x*y) || x/sin(x) - y/sin(y) == -(x*y/sin(x*y))"
    ```

  - 📄 Hair.gqs 🐌

    ```text
    "sin((x + sin(y)) * (sin(x) + y)) == cos(sin((sin(x) + cos(y)) * (sin(y) + cos(x)))) || \
     sin((x + sin(y)) * (sin(x) + y)) == cos(sin((sin(x) + cos(y)) * (sin(y) - cos(x)))) || \
     sin((x + sin(y)) * (sin(x) + y)) == cos(sin((sin(x) - cos(y)) * (sin(y) + cos(x)))) || \
     sin((x + sin(y)) * (sin(x) + y)) == cos(sin((sin(x) - cos(y)) * (sin(y) - cos(x)))) || \
     sin((x + sin(y)) * (sin(x) - y)) == cos(sin((sin(x) + cos(y)) * (sin(y) + cos(x)))) || \
     sin((x + sin(y)) * (sin(x) - y)) == cos(sin((sin(x) + cos(y)) * (sin(y) - cos(x)))) || \
     sin((x + sin(y)) * (sin(x) - y)) == cos(sin((sin(x) - cos(y)) * (sin(y) + cos(x)))) || \
     sin((x + sin(y)) * (sin(x) - y)) == cos(sin((sin(x) - cos(y)) * (sin(y) - cos(x)))) || \
     sin((x - sin(y)) * (sin(x) + y)) == cos(sin((sin(x) + cos(y)) * (sin(y) + cos(x)))) || \
     sin((x - sin(y)) * (sin(x) + y)) == cos(sin((sin(x) + cos(y)) * (sin(y) - cos(x)))) || \
     sin((x - sin(y)) * (sin(x) + y)) == cos(sin((sin(x) - cos(y)) * (sin(y) + cos(x)))) || \
     sin((x - sin(y)) * (sin(x) + y)) == cos(sin((sin(x) - cos(y)) * (sin(y) - cos(x)))) || \
     sin((x - sin(y)) * (sin(x) - y)) == cos(sin((sin(x) + cos(y)) * (sin(y) + cos(x)))) || \
     sin((x - sin(y)) * (sin(x) - y)) == cos(sin((sin(x) + cos(y)) * (sin(y) - cos(x)))) || \
     sin((x - sin(y)) * (sin(x) - y)) == cos(sin((sin(x) - cos(y)) * (sin(y) + cos(x)))) || \
     sin((x - sin(y)) * (sin(x) - y)) == cos(sin((sin(x) - cos(y)) * (sin(y) - cos(x))))" \
    -b 4.0 6.5 2.0 4.5
    ```

  - 📄 Highwire.gqs 🐌

    ```text
    "abs(x*cos(x) - y*sin(y)) == abs(x*cos(y) - y*sin(x))"
    ```

  - 📄 Trapezoidal Fortress.gqs 🐌

    ```text
    "abs(x*cos(x) + y*sin(y)) == x*cos(y) - y*sin(x)"
    ```

- 📂 Single Relation/Solid/

  - 📄 Sharp Threesome.gqs

    ```text
    "(sin(sqrt((x + 5)^2 + y^2))) * (cos(8*atan(y/(x + 5)))) * (sin(sqrt((x - 5)^2 + (y - 5)^2))) * \
     (cos(8*atan((y - 5)/(x - 5)))) * (sin(sqrt(x^2 + (y + 5)^2))) * (cos(8*atan((y + 5)/x))) > 0"
    ```

  - 📄 The Disco Hall.gqs

    ```text
    "sin(abs(x + y)) > max(cos(x^2), sin(y^2))"
    ```

## Examples from [GrafEq Reviews](http://www.peda.com/grafeq/reviews.html)

```text
"y == sqrt(x)^2"
```

```text
"y == sqrt(x - 1)/sqrt(x - 3)"
```

The graph must be empty:

```text
"y == sqrt(x - 3)*sqrt(1 - x)"
```

## Examples from [Cool Graphs of Implicit Equations](https://web.archive.org/web/20160221140058/http://www.xamuel.com/graphs-of-implicit-equations/)

🐌

```text
"exp(sin(x) + cos(y)) == sin(exp(x + y))"
```

```text
"sin(sin(x) + cos(y)) == cos(sin(x*y) + cos(x))"
```

```text
"sin(x^2 + y^2) == cos(x*y)"
```

```text
"abs(sin(x^2 - y^2)) == sin(x + y) + cos(x * y)"
```

```text
"abs(sin(x^2 + 2*x*y)) == sin(x - 2*y)"
```

## Tests for Conjunction and Disjunction

The graph must be empty:

```text
"y == x && y == x + 0.0001"
```

The graph must not be empty:

```text
"y == x || y == x + 0.0001"
```

```text
'y < sqrt(x) && y < sqrt(-x)'
```

```text
'y < sqrt(x) || y < sqrt(-x)'
```

```text
"y == sin(40/x) && (x > 0 && y > 0)"
```

```text
"y == sin(40/x) && (x > 0 || y > 0)"
```

```text
"y == sin(40/x) || (x > 0 && y > 0)"
```

```text
"y == sin(40/x) || (x > 0 || y > 0)"
```
