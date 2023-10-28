# wg-vanity-key

Generate vanity Wireguard private keys

This will fully utilize your CPU by randomly generating private keys and
calculating their public keys. If a public key matches any of the given strings,
save it.

## Installation

```
$ cargo install --git=https://github.com/darkwater/wg-vanity-key
```

## Usage

```
$ wg-vanity-key a+ b+ c+ >>keys.txt
found one! a+5kyVRK4zAbyU3kWAcGbGc4W54zczW1LR3xZNuwkw0=
found one! c+9perPeQn5ilzJDh16OfRZT2rkl8XxryxjzCyfOfX4=
found one! b+DzLSuBXuBDmxr5U4jn6v7ZgjmT1d+7ClbMdjGH4kM=
All keys found!

$ cat keys.txt
sk: nv3q9IlO2dhdlT0DxH+4xCIqX7SD6iYXpHBv3H2DRTI= pk: a+5kyVRK4zAbyU3kWAcGbGc4W54zczW1LR3xZNuwkw0=
sk: IQlCdVJx/Bvr86qurkhB5NAisE17tFNUuvHE7F+ujGM= pk: c+9perPeQn5ilzJDh16OfRZT2rkl8XxryxjzCyfOfX4=
sk: s9FOmazi/nrAUfWdFypalHU3pc03slcCV8hD4H/ArHY= pk: b+DzLSuBXuBDmxr5U4jn6v7ZgjmT1d+7ClbMdjGH4kM=
```

## Performance

On my 12-core 24-thread Ryzen 5900X, it's going through about 14,000 keys per
second, or 840,000 keys per minute. Since keys are base64, assuming all
characters are equally likely in each position, it would take on average 64^N
tries to find a key with prefix of length N.
