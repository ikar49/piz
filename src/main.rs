#![allow(unused)]

use std::{fs, io};
use std::io::{Read, Seek, SeekFrom, Write};

const PIZ_LABEL: [u8; 3] = ['P' as u8, 'i' as u8, 'Z' as u8];
const HEX_LABEL: [u8; 3] = ['H' as u8, 'e' as u8, 'x' as u8];
const DEC_LABEL: [u8; 3] = ['D' as u8, 'e' as u8, 'c' as u8];
const OCT_LABEL: [u8; 3] = ['O' as u8, 'c' as u8, 't' as u8];
const BIN_LABEL: [u8; 3] = ['B' as u8, 'i' as u8, 'n' as u8];

const CR : u8 = '\r' as u8;
const LF : u8 = '\n' as u8;
const TAB: u8 = '\t' as u8;

#[derive(Debug, Clone, Copy, PartialEq)]
enum NumberBase {
    Hex,
    Dec,
    Oct,
    Bin,
    Num(u64),
}

impl NumberBase {
    fn label(&self) -> &[u8] {
        match self {
            NumberBase::Hex => &HEX_LABEL,
            NumberBase::Dec => &DEC_LABEL,
            NumberBase::Oct => &OCT_LABEL,
            NumberBase::Bin => &BIN_LABEL,
            _ => todo!(),
        }
    }
}

fn main() -> io::Result<()> {
    // todo: Убрать этот хардкод
    let archive_name = "example.piz";
    let output_name  = "example.out";

    let mut file = fs::File::open(&archive_name)?;

    let (read_base, calc_base) = parse_header(&mut file)?;
    let mut bytes = file.bytes();

    // Пропускаем метаданные
    let mut data_byte = loop {
        let byte = bytes.next().unwrap()?;
        if byte != TAB {
            break byte;
        }
        // Пропускаем всю строку
        while bytes.next().unwrap()? != LF {}
    };

    // Учитываем базу чисел, записанных в файл (ха-ха)
    if read_base != NumberBase::Dec {
        todo!();
    }

    let storage = parse_to_end(data_byte, bytes)?;

    // Учитываем базу чисел для расчётов (ха-ха)
    if calc_base != NumberBase::Hex {
        todo!();
    }

    println!("read: {:?}\ncalc: {:?}", read_base, calc_base);
    let mut file = fs::OpenOptions::new()
        .read(false)
        .write(true)
        .append(false)
        .truncate(true)
        .create(true)
        .open(output_name)?;

    for (length, position_vec) in storage {
        for position in position_vec {
            let length = length.clone();
            let bytes = get_bytes_from_pi(position, length);
            file.write(&bytes)?;
        }
    }

    Ok(())
}

fn parse_header(file: &mut (impl Read + Seek)) -> io::Result<(NumberBase, NumberBase)> {
    // header гарантированно должен вмещать каждый *_LABEL
    let mut header = [0u8; PIZ_LABEL.len() + 1];
    file.read(&mut header)?;

    if &header[..PIZ_LABEL.len()] != &PIZ_LABEL {
        panic!("Incorrect piz archive!");
    }

    // todo: Наверное стоит сделать рекурсивной функцией?
    let bases = match header[PIZ_LABEL.len()] {
        TAB => {
            let n = file.read(&mut header)?;
            let read_base = parse_base_label(&header);
            file.seek(SeekFrom::Current(read_base.label().len() as i64 - n as i64))?;

            file.read(&mut header[..1])?;
            let calc_base = match header[0] {
                TAB => {
                    let n = file.read(&mut header)?;
                    let calc_base = parse_base_label(&header);
                    file.seek(SeekFrom::Current(calc_base.label().len() as i64 - n as i64));

                    file.read(&mut header[..1])?;
                    if header[0] != LF {
                        panic!("Incorrect piz archive!");
                    }

                    calc_base
                },
                LF => read_base.clone(),
                _  => panic!("Incorrect piz archive!"),
            };

            (read_base, calc_base)
        },
        LF => (NumberBase::Dec, NumberBase::Dec),
        _  => panic!("Incorrect piz archive!"),
    };

    Ok(bases)
}

fn parse_base_label(header: &[u8]) -> NumberBase {
    if &header[..HEX_LABEL.len()] == &HEX_LABEL {
        NumberBase::Hex
    } else if &header[..DEC_LABEL.len()] == &DEC_LABEL {
        NumberBase::Dec
    } else if &header[..OCT_LABEL.len()] == &OCT_LABEL {
        NumberBase::Oct
    } else if &header[..BIN_LABEL.len()] == &BIN_LABEL {
        NumberBase::Bin
    } else {
        todo!();
    }
}

fn parse_to_end<R>(mut data_byte: u8, mut bytes: io::Bytes<R>) -> io::Result<Vec<(u32, Vec<u32>)>>
    where R: io::Read
{
    let mut storage = Vec::new();

    loop {
        // Считываем число, описывающее длину искомой последовательности
        let mut block_length = 0u32;   // todo: Избавиться от ограничения в 32 бита
        while data_byte != LF {
            block_length *= 10;
            block_length += (data_byte - '0' as u8) as u32;
            data_byte = bytes.next().unwrap()?;
        }
        storage.push((block_length, Vec::new()));

        // Считываем все числа позиций этой группы
        let storage_elem = storage.last_mut().unwrap();
        loop {
            data_byte = bytes.next().unwrap()?;
            if data_byte != TAB {
                // Группа закончилась
                break;
            }

            // Дочитываем строку до конца
            let mut current_position = 0u32;
            for pos_byte in bytes.by_ref() {
                let pos_byte = pos_byte?;
                if pos_byte == LF {
                    break;
                }
                current_position *= 10;
                current_position += (pos_byte - '0' as u8) as u32;
            }
            storage_elem.1.push(current_position);
        }

        // Два LF = конец файла
        if data_byte == LF {
            break;
        }
    }

    Ok(storage)
}

fn get_bytes_from_pi(pos: u32, length: u32) -> Vec<u8> {

    // Байты хранятся в "сетевом" виде, так как с таким порядком удобнее работать "на бумаге"
    // То есть порядок записи - big endian.
    // То есть строка "jk" = 0x6A 0x6B
    // Дополнительно уточним: учитывая, что мы записываем байты в виде чисел в некоторой системе
    // счисления, то один байт так же может состоять их нескольких цифр.
    // Например из двух шестнадцатеричных.
    // В таком случае эти цифры мы тоже записываем в big-endian порядке.
    // То есть байт 'j' = 0x6 0xA

    // Так как пока считаем только в шестнадцатеричной системе, то будем искать по 2 цифры,
    // так как один байт содержит две цифры. Да, не оптимально (точности хватит и реже считать),
    // но удобно ибо понятно, что length % 2 == 0

    let mut bytes = Vec::new();

    for idx in (0..length).step_by(2) {
        let s1 = series::<1>(pos + idx);
        let s4 = series::<4>(pos + idx);
        let s5 = series::<5>(pos + idx);
        let s6 = series::<6>(pos + idx);

        let pid = 4_f64 * s1 - 2_f64 * s4 - s5 - s6;
        let pid = pid.fract() + 1_f64;
        let pid = pid.abs(); // Кажется, что не нужно, так как когда мы берём {pid} мы уже получаем
                             // число в диапазоне (-1; 1), и прибавляя к нему 1 гарантированно
                             // получим положительное число в диапазоне (0; 2).

        let pid0 = (16_f64 * pid.fract());
        let pid1 = (16_f64 * pid0.fract());

        let byte0 = ((pid0 as usize) << 4) as u8; // старшая часть байта
        let byte1 = ((pid1 as usize) << 0) as u8; // младшая часть байта

        bytes.push(byte0 | byte1);
    }

    bytes
}

fn series<const N: u32>(position: u32) -> f64 {
    let mut sum = 0_f64;

    for k in 0..position {
        let ak = 8 * k + N;
        let p = position - k;
        let t = modulo(16_i64.pow(p), ak as i64);
        let b = t as f64 / ak as f64;
        sum = (sum + b).fract();
    }

    const M: i32 = 100;

    let position = position as i64;
    for k in position..=(position + M as i64) {
        let ak = 8 * k + N as i64;
        let p = position - k;   // -M <= p <= 0
        let t = 16_f64.powi(p as i32);
        let b = t / ak as f64;
        sum = (sum + b).fract();
    }

    sum
}

// a mod b
// Подсчёт деления по модулю очень упрощённый для конкретной задачи
// В качестве a попадает всегда только положительное число
// В случае если b тоже положительное - уходим к подсчёту обычного остатка (a % b)
// В противном случае - считаем по упрощённой формуле b + (a % |b|)
fn modulo(a: i64, b: i64) -> i64 {
    if b == 0 {
        panic!("div by zero");
    } else if b > 0 {
        a % b
    } else {
        b + (a % b.abs())
    }
}

fn find_in_pi(data: &[u8]) -> usize {
    0
}
