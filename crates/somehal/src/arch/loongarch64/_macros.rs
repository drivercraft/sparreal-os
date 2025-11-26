macro_rules! ext_sym_addr {
    ($sym:expr) => {
        {
            #[allow(unused_unsafe)]
            unsafe{
                let out: usize;
                core::arch::asm!(
                    concat!("la.pcrel    {r}, ", stringify!($sym)),
                    r = out(reg) out,
                );
                out
            }
        }
    };
}

macro_rules! backup_t0t1 {
    () => {
        // KS0=0x30, KS1=0x31
        "
        csrwr  $t0, 0x30
        csrwr  $t1, 0x31
        "
    };
}

macro_rules! restore_t0t1 {
    () => {
        // KS0=0x30, KS1=0x31
        "
        csrrd  $t0, 0x30
        csrrd  $t1, 0x31
        "
    };
}

macro_rules! st_d {
    ($rd:expr, $rj:expr, $off:expr) => {
        concat!(
            "st.d    ",
            stringify!($rd),
            ", ",
            stringify!($rj),
            ", ",
            stringify!($off),
            " * 8"
        )
    };
}

macro_rules! op_general_regs {
    ($op:expr) => {
        concat!(
            $op,
            " $ra, $sp, 8 * 1\n",
            $op,
            " $a0, $sp, 8 * 4\n",
            $op,
            " $a1, $sp, 8 * 5\n",
            $op,
            " $a2, $sp, 8 * 6\n",
            $op,
            " $a3, $sp, 8 * 7\n",
            $op,
            " $a4, $sp, 8 * 8\n",
            $op,
            " $a5, $sp, 8 * 9\n",
            $op,
            " $a6, $sp, 8 * 10\n",
            $op,
            " $a7, $sp, 8 * 11\n",
            $op,
            " $t0, $sp, 8 * 12\n",
            $op,
            " $t1, $sp, 8 * 13\n",
            $op,
            " $t2, $sp, 8 * 14\n",
            $op,
            " $t3, $sp, 8 * 15\n",
            $op,
            " $t4, $sp, 8 * 16\n",
            $op,
            " $t5, $sp, 8 * 17\n",
            $op,
            " $t6, $sp, 8 * 18\n",
            $op,
            " $t7, $sp, 8 * 19\n",
            $op,
            " $t8, $sp, 8 * 20\n",
            $op,
            " $fp, $sp, 8 * 22\n",
            $op,
            " $s0, $sp, 8 * 23\n",
            $op,
            " $s1, $sp, 8 * 24\n",
            $op,
            " $s2, $sp, 8 * 25\n",
            $op,
            " $s3, $sp, 8 * 26\n",
            $op,
            " $s4, $sp, 8 * 27\n",
            $op,
            " $s5, $sp, 8 * 28\n",
            $op,
            " $s6, $sp, 8 * 29\n",
            $op,
            " $s7, $sp, 8 * 30\n",
            $op,
            " $s8, $sp, 8 * 31\n"
        )
    };
}

macro_rules! push_general_regs {
    () => {
        op_general_regs!("st.d")
    };
}

macro_rules! pop_general_regs {
    () => {
        op_general_regs!("ld.d")
    };
}
