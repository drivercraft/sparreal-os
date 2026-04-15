#![no_std]
#![no_main]
#![feature(used_with_arg)]

#[macro_use]
extern crate alloc;
extern crate bare_test;

#[bare_test::tests]
mod tests {
    use alloc::{
        string::{String, ToString},
        vec::Vec,
    };
    use bare_test::irq::{IrqHandleResult, IrqParam};
    use core::{
        cell::UnsafeCell,
        fmt::Debug,
        ptr::NonNull,
        sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    };
    use rdif_serial::{BIrqHandler, BReciever, BSender, BSerial, TransferError};

    use super::*;
    use bare_test::{
        globals::{global_val, PlatformInfoKind},
        irq::IrqInfo,
        mem::iomap,
        GetIrqConfig,
    };
    use log::{debug, info};
    use some_serial::{DataBits, InterruptMask, Parity, StopBits};

    // === 全局中断计数器 ===

    /// 发送中断计数器
    static TX_INTERRUPT_COUNT: AtomicUsize = AtomicUsize::new(0);

    /// 接收中断计数器
    static RX_INTERRUPT_COUNT: AtomicUsize = AtomicUsize::new(0);

    /// 中断处理函数计数器（用于调试）
    static IRQ_HANDLER_CALL_COUNT: AtomicUsize = AtomicUsize::new(0);

    #[derive(Debug)]
    struct SInfo {
        base: NonNull<u8>,
        clk: u32,
        #[allow(dead_code)]
        irq: IrqInfo,
    }

    // === 中断测试辅助函数 ===

    /// 重置所有中断计数器
    #[allow(dead_code)]
    fn reset_interrupt_counters() {
        TX_INTERRUPT_COUNT.store(0, Ordering::SeqCst);
        RX_INTERRUPT_COUNT.store(0, Ordering::SeqCst);
        IRQ_HANDLER_CALL_COUNT.store(0, Ordering::SeqCst);
        info!("✓ Interrupt counters reset");
    }

    /// 获取当前中断计数
    #[allow(dead_code)]
    fn get_interrupt_counts() -> (usize, usize, usize) {
        let tx_count = TX_INTERRUPT_COUNT.load(Ordering::SeqCst);
        let rx_count = RX_INTERRUPT_COUNT.load(Ordering::SeqCst);
        let handler_count = IRQ_HANDLER_CALL_COUNT.load(Ordering::SeqCst);
        (tx_count, rx_count, handler_count)
    }

    /// 打印中断计数状态
    #[allow(dead_code)]
    fn print_interrupt_counts(context: &str) {
        let (tx_count, rx_count, handler_count) = get_interrupt_counts();
        info!(
            "IRQ counts [{}]: TX={}, RX={}, Handler={}",
            context, tx_count, rx_count, handler_count
        );
    }

    fn clean_rx(rx: &mut BReciever) {
        let mut buff = [0u8; 64];
        loop {
            match rx.read_bytes(&mut buff) {
                Ok(0) => break,
                Ok(_) => {}
                Err(_) => break,
            }
        }
    }

    // === Serial 专用回环测试 ===

    /// Serial 基础回环测试 - 验证 Interface trait 基本功能
    #[test]
    fn test_serial_basic_loopback() {
        info!("=== Serial Basic Loopback Test ===");

        let mut serial = create_test_serial();

        serial.open().expect("Failed to open Serial");

        info!("Testing PL011 driver with TX/RX interfaces");

        // 获取 TX/RX 接口
        let mut tx = match serial.take_tx() {
            Some(tx) => {
                info!("✓ TX interface obtained successfully");
                tx
            }
            None => {
                info!("✗ Failed to obtain TX interface");
                return;
            }
        };

        let mut rx = match serial.take_rx() {
            Some(rx) => {
                info!("✓ RX interface obtained successfully");
                rx
            }
            None => {
                info!("✗ Failed to obtain RX interface");
                return;
            }
        };

        clean_rx(&mut rx);

        // 测试回环功能
        let test_data = b"Hello\n";
        info!("Testing loopback with data: {test_data:?}");
        test_serial_tx_rx_one(&mut serial, &mut tx, &mut rx, test_data)
            .expect("loopback should succeed");
        info!("✓ Loopback test passed");

        // 清理资源
        drop(tx);
        drop(rx);

        info!("=== Serial Basic Loopback Test Completed ===");
    }

    /// 中断掩码控制测试
    #[test]
    fn test_interrupt_mask_control() {
        info!("=== Interrupt Mask Control Test ===");

        let mut serial = create_test_serial();
        serial.open().expect("Failed to open serial");

        info!("Testing interrupt mask control for PL011 driver");

        // 重置中断计数器
        reset_interrupt_counters();
        print_interrupt_counts("initial");

        // 测试1：仅启用TX中断
        info!("Test 1: Enable TX interrupt only");
        serial.enable_interrupts(InterruptMask::TX_EMPTY);
        let mut tx = serial.take_tx().unwrap();

        let mut bytes = b"TX mask test".as_slice();
        while !bytes.is_empty() {
            let n = tx.write_bytes(bytes);
            bytes = &bytes[n..];
        }

        // 等待中断处理
        for _ in 0..50000 {
            core::hint::spin_loop();
        }

        let (tx_count1, rx_count1, _) = get_interrupt_counts();
        serial.disable_loopback();
        info!("After TX-only: TX={}, RX={}", tx_count1, rx_count1);

        serial.disable_interrupts(InterruptMask::TX_EMPTY | InterruptMask::RX_AVAILABLE);
        reset_interrupt_counters();
        let mut rx = serial.take_rx().unwrap();
        clean_rx(&mut rx);
        // Note: clean_fifo() method is not available in the new interface
        // FIFO is automatically managed by the hardware driver

        // 测试2：仅启用RX中断
        info!("Test 2: Enable RX interrupt only");
        let mut bytes = b"TX mask test".as_slice();
        info!("Sending data: {:?}", bytes);

        serial.enable_interrupts(InterruptMask::RX_AVAILABLE);
        serial.enable_loopback();

        let n = bytes.len();

        while !bytes.is_empty() {
            let n = tx.write_bytes(bytes);
            bytes = &bytes[n..];
        }

        // 等待中断处理
        for _ in 0..50000 {
            core::hint::spin_loop();
        }

        let mut buff = vec![0u8; n + 20];

        let rn = rx.read_bytes(&mut buff).unwrap();

        let (tx_count2, rx_count2, _) = get_interrupt_counts();
        serial.disable_loopback();
        info!(
            "After RX-only: TX={}, RX={}, send {}",
            tx_count2, rx_count2, n
        );
        info!("Received data: {:?}", &buff[..rn]);

        // 清理
        serial.disable_interrupts(InterruptMask::TX_EMPTY | InterruptMask::RX_AVAILABLE);
        // Note: clean_fifo() method is not available in the new interface
        // FIFO is automatically managed by the hardware driver
        reset_interrupt_counters();
        clean_rx(&mut rx);

        // 测试3：启用TX和RX中断
        info!("Test 3: Enable both TX and RX interrupts");
        serial.enable_interrupts(InterruptMask::TX_EMPTY | InterruptMask::RX_AVAILABLE);
        serial.enable_loopback();
        let _ = tx.write_bytes(b"Both mask test");

        // 等待数据通过回环传输到RX FIFO并触发中断
        // 在读取数据之前让中断处理程序有机会检测到RX中断
        for _i in 0..80000 {
            core::hint::spin_loop();
        }

        let (tx_count3, rx_count3, _) = get_interrupt_counts();
        serial.disable_loopback();
        info!("After both: TX={}, RX={}", tx_count3, rx_count3);

        // 验证掩码控制有效性
        if tx_count1 > 0 && rx_count1 == 0 {
            info!("✓ TX-only mask test passed");
        } else {
            panic!("✗ TX-only mask test failed");
        }

        if tx_count2 == 0 && rx_count2 > 0 {
            info!("✓ RX-only mask test passed");
        } else {
            panic!("✗ RX-only mask test failed");
        }

        if tx_count3 > 0 && rx_count3 > 0 {
            info!("✓ Both interrupts mask test passed");
        } else {
            panic!("✗ Both interrupts mask test failed");
        }

        // 最终清理
        serial.disable_interrupts(InterruptMask::TX_EMPTY | InterruptMask::RX_AVAILABLE);
        info!("✓ PL011 interrupt mask control test completed");

        info!("=== Interrupt Mask Control Test Completed ===");
    }

    // /// Serial 压力测试 - 多次连续回环操作
    // #[test]
    // fn test_serial_stress_loopback() {
    //     info!("=== Serial Stress Loopback Test ===");

    //     let mut serial = create_test_serial();
    //     serial.open().expect("Failed to open Serial");

    //     let config = some_serial::Config::new()
    //         .baudrate(115200)
    //         .data_bits(DataBits::Eight)
    //         .stop_bits(StopBits::One)
    //         .parity(Parity::None);

    //     if let Err(e) = serial.set_config(&config) {
    //         info!("Config failed: {:?}", e);
    //         return;
    //     }

    //     serial.enable_loopback();

    //     let stress_data = b"Stress test data for Serial interface";
    //     let iterations = 10;
    //     let mut successful_iterations = 0;

    //     for i in 0..iterations {
    //         info!("Stress iteration {}/{}", i + 1, iterations);

    //         let mut tx = match serial.take_tx() {
    //             Some(tx) => tx,
    //             None => {
    //                 info!("Failed to get TX in iteration {}", i + 1);
    //                 continue;
    //             }
    //         };

    //         let mut rx = match serial.take_rx() {
    //             Some(rx) => rx,
    //             None => {
    //                 info!("Failed to get RX in iteration {}", i + 1);
    //                 continue;
    //             }
    //         };

    //         if test_serial_loopback_with_data(&mut serial, &mut tx, &mut rx, stress_data) {
    //             successful_iterations += 1;
    //         }

    //         // 添加短暂延迟
    //         for _ in 0..1000 {
    //             core::hint::spin_loop();
    //         }
    //     }

    //     info!(
    //         "Stress test completed: {}/{} iterations successful",
    //         successful_iterations, iterations
    //     );

    //     serial.disable_loopback();

    //     info!("=== Serial Stress Loopback Test Completed ===");
    // }

    // === Serial 专用辅助函数 ===

    /// 支持的 UART 兼容性列表和对应的驱动类型
    #[derive(Debug)]
    enum UartDriverType {
        PL011,
        Ns16550Mmio,
    }

    /// 从兼容性字符串获取对应的驱动类型
    fn get_driver_type_from_compatible(compatible: &[String]) -> Option<UartDriverType> {
        for comp in compatible {
            match comp.as_str() {
                "arm,pl011" => return Some(UartDriverType::PL011),
                "snps,dw-apb-uart" => return Some(UartDriverType::Ns16550Mmio), // DesignWare APB UART 兼容 NS16550
                _ => {}
            }
        }
        None
    }

    /// 创建统一的测试用 Serial 实例，支持多种驱动类型
    fn create_test_serial() -> BSerial {
        let uart_info =
            find_best_uart_for_testing().expect("No suitable UART device found for testing");

        info!(
            "Creating test serial with device type: {:?}",
            uart_info.driver_type
        );

        let mut uart: BSerial = match uart_info.driver_type {
            UartDriverType::PL011 => {
                some_serial::pl011::Pl011::new_boxed(uart_info.base, uart_info.clk)
            }
            UartDriverType::Ns16550Mmio => {
                some_serial::ns16550::Ns16550::new_mmio_boxed(uart_info.base, uart_info.clk, 4)
            }
        };

        uart.open().expect("Failed to initialize UART");

        if let Some(handler) = uart.irq_handler() {
            register_irq(&uart_info.irq, handler);
        }

        uart
    }

    /// UART 设备信息
    struct UartDeviceInfo {
        base: core::ptr::NonNull<u8>,
        clk: u32,
        irq: bare_test::irq::IrqInfo,
        driver_type: UartDriverType,
    }

    /// 查找最适合测试的 UART 设备
    fn find_best_uart_for_testing() -> Option<UartDeviceInfo> {
        let PlatformInfoKind::DeviceTree(fdt) = &global_val().platform_info;
        let fdt = fdt.get();
        let node = fdt.chosen().unwrap().debugcon().unwrap();
        let mut compatible = vec![];
        for comp in node.compatibles() {
            compatible.push(comp.to_string());
        }

        info!("Chosen debug console compatible: {:?}", compatible);

        // 提取设备信息
        let driver_type = get_driver_type_from_compatible(&compatible).expect("驱动类型应该已知");

        let addr = node.reg().unwrap().next().unwrap();
        let size = addr.size.unwrap_or(0x1000).max(0x1000);
        let irq_info = node.irq_info().unwrap();
        let base = iomap((addr.address as usize).into(), size);

        let clk = node
            .clocks()
            .next()
            .and_then(|clk| clk.clock_frequency)
            .unwrap_or({
                // 根据设备类型设置默认时钟频率
                match driver_type {
                    UartDriverType::PL011 => 24_000_000,
                    UartDriverType::Ns16550Mmio => 1_843_200,
                }
            });

        info!("UART 设备信息:");
        info!("  地址: 0x{:x}", addr.address);
        info!("  时钟: {} Hz", clk);
        info!("  驱动类型: {:?}", driver_type);

        Some(UartDeviceInfo {
            base,
            clk,
            irq: irq_info,
            driver_type,
        })
    }

    /// Serial 回环数据测试函数
    fn test_serial_tx_rx_one(
        s: &mut BSerial,
        tx: &mut BSender,
        rx: &mut BReciever,
        test_data: &[u8],
    ) -> Result<(), TransferError> {
        let mut rcv_buff = Vec::with_capacity(64);
        s.enable_loopback();
        for _ in 0..10000 {
            core::hint::spin_loop();
        }
        clean_rx(rx);
        // 尽量清空残留数据，避免与本次测试混淆
        // Note: clean_fifo() method is not available in the new interface
        // FIFO is automatically managed by the hardware driver
        let mut retry = 0;
        const RETRY: usize = 10000;
        let mut buff = [0u8; 1];
        for (i, b) in test_data.iter().enumerate() {
            // 发送一个字节
            for _ in 0..RETRY {
                let sent = tx.write_bytes(&[*b]);
                if sent == 1 {
                    break;
                } else {
                    retry += 1;
                    if retry >= RETRY {
                        s.disable_loopback();
                        panic!("Failed to send byte {} after {} retries", i, RETRY);
                    }
                }
            }
            retry = 0;
            // 接收一个字节
            for _ in 0..RETRY {
                match rx.read_bytes(&mut buff) {
                    Ok(1) => {
                        rcv_buff.push(buff[0]);
                        break;
                    }
                    Ok(0) => {
                        retry += 1;
                        if retry >= RETRY {
                            s.disable_loopback();
                            panic!("Failed to receive byte {} after {} retries", i, RETRY);
                        }
                    }
                    Ok(_) => {
                        // Received more than expected, just take the first byte
                        rcv_buff.push(buff[0]);
                        break;
                    }
                    Err(e) => {
                        s.disable_loopback();
                        panic!("Receive error on byte {}: {:?}", i, e);
                    }
                }
            }
        }
        s.disable_loopback();
        assert_eq!(&rcv_buff, test_data);
        info!("✓ Data matched: received={:?}", rcv_buff);

        Ok(())
    }

    /// 测试 Serial 配置功能
    #[allow(dead_code)]
    fn test_serial_configuration(serial: &mut BSerial) {
        info!("Testing Serial configuration...");

        let test_configs = [
            (115200, DataBits::Eight, StopBits::One, Parity::None),
            (9600, DataBits::Seven, StopBits::One, Parity::Even),
            (38400, DataBits::Eight, StopBits::Two, Parity::Odd),
        ];

        for (i, (baudrate, data_bits, stop_bits, parity)) in test_configs.iter().enumerate() {
            let config = some_serial::Config::new()
                .baudrate(*baudrate)
                .data_bits(*data_bits)
                .stop_bits(*stop_bits)
                .parity(*parity);

            if let Err(e) = serial.set_config(&config) {
                info!("Config {} failed: {:?}", i + 1, e);
                continue;
            }

            // 验证配置
            let actual_baudrate = serial.baudrate();
            let actual_data_bits = serial.data_bits();
            let actual_stop_bits = serial.stop_bits();
            let actual_parity = serial.parity();

            info!("Config {} applied:", i + 1);
            info!("  Baudrate: {} (expected: {})", actual_baudrate, baudrate);
            info!(
                "  Data bits: {:?} (expected: {:?})",
                actual_data_bits, data_bits
            );
            info!(
                "  Stop bits: {:?} (expected: {:?})",
                actual_stop_bits, stop_bits
            );
            info!("  Parity: {:?} (expected: {:?})", actual_parity, parity);
        }

        info!("✓ Serial configuration test completed");
    }

    /// 测试 Serial 回环控制功能
    #[allow(dead_code)]
    fn test_serial_loopback_control(serial: &mut BSerial) {
        info!("Testing Serial loopback control...");

        // 初始状态
        let initial_state = serial.is_loopback_enabled();
        info!("Initial loopback state: {}", initial_state);

        // 启用回环
        serial.enable_loopback();
        assert!(serial.is_loopback_enabled());
        info!("✓ Loopback enable verified");

        // 禁用回环
        serial.disable_loopback();
        assert!(!serial.is_loopback_enabled());
        info!("✓ Loopback disable verified");

        // 恢复初始状态
        if initial_state {
            serial.enable_loopback();
        }

        info!("✓ Serial loopback control test completed");
    }

    /// 测试 Serial 中断管理功能
    #[allow(dead_code)]
    fn test_serial_interrupt_management(serial: &mut BSerial) {
        info!("Testing Serial interrupt management...");

        let test_masks = [
            InterruptMask::RX_AVAILABLE,
            InterruptMask::TX_EMPTY,
            InterruptMask::RX_AVAILABLE | InterruptMask::TX_EMPTY,
        ];

        for (i, mask) in test_masks.iter().enumerate() {
            info!("Testing mask {}: {:?}", i + 1, mask);

            // 启用中断
            serial.enable_interrupts(*mask);
            info!("✓ Interrupts enabled for mask {}", i + 1);

            // 禁用中断
            serial.disable_interrupts(*mask);
            info!("✓ Interrupts disabled for mask {}", i + 1);
        }

        info!("✓ Serial interrupt management test completed");
    }

    /// 测试 Serial DriverGeneric 接口
    #[allow(dead_code)]
    fn test_serial_driver_generic(serial: &mut BSerial) {
        info!("Testing Serial DriverGeneric interface...");

        // 测试 open/close
        serial.open().expect("Failed to open serial");
        info!("✓ Serial open successful");

        let _ = serial.close();
        info!("✓ Serial close successful");

        // 测试 base 地址获取
        let base_addr = serial.base_addr();
        info!("✓ Serial base address: 0x{:x}", base_addr);

        info!("✓ Serial DriverGeneric interface test completed");
    }

    // === 现有辅助函数（保持不变） ===
    fn get_uart(cmp: &[&str]) -> SInfo {
        let PlatformInfoKind::DeviceTree(fdt) = &global_val().platform_info;
        let fdt = fdt.get();
        let node = fdt.find_compatible(cmp).next().unwrap();

        let addr = node.reg().unwrap().next().unwrap();
        let size = addr.size.unwrap_or(0x1000);
        let irq_info = node.irq_info().unwrap();
        let base = iomap((addr.address as usize).into(), size);
        let clk = node.clocks().next().unwrap().clock_frequency.unwrap();

        SInfo {
            base,
            clk,
            irq: irq_info,
        }
    }

    #[allow(dead_code)]
    fn detect_uart_devices() {
        let PlatformInfoKind::DeviceTree(fdt) = &global_val().platform_info;
        let fdt = fdt.get();

        let pl011_devices = fdt.find_compatible(&["arm,pl011"]).collect::<Vec<_>>();

        info!("=== UART Device Detection ===");
        info!("Found {} PL011 devices in device tree", pl011_devices.len());

        for (i, node) in pl011_devices.iter().enumerate() {
            if let Some(addr) = node.reg().and_then(|mut reg| reg.next()) {
                info!("  PL011 {}: address 0x{:x}", i, addr.address);
            } else {
                info!("  PL011 {}: no address found", i);
            }
        }
        info!("=== End Device Detection ===");
    }

    fn get_secondary_uart() -> Option<SInfo> {
        let PlatformInfoKind::DeviceTree(fdt) = &global_val().platform_info;
        let fdt = fdt.get();

        let pl011_devices = fdt.find_compatible(&["arm,pl011"]).collect::<Vec<_>>();

        if pl011_devices.len() > 1 {
            let node = &pl011_devices[1];

            let addr = node.reg().unwrap().next().unwrap();
            let size = addr.size.unwrap_or(0x1000);
            let irq_info = node.irq_info().unwrap();
            let base = iomap((addr.address as usize).into(), size);

            let clk = node
                .clocks()
                .next()
                .and_then(|clk| clk.clock_frequency)
                .unwrap_or(24_000_000);

            info!(
                "Using secondary PL011 at address 0x{:x}, clock: {} Hz",
                addr.address, clk
            );

            Some(SInfo {
                base,
                clk,
                irq: irq_info,
            })
        } else {
            info!(
                "No secondary PL011 device found, only {} PL011 devices available",
                pl011_devices.len()
            );
            None
        }
    }

    struct HWIrqHandler(UnsafeCell<Option<BIrqHandler>>);
    unsafe impl Sync for HWIrqHandler {}
    unsafe impl Send for HWIrqHandler {}

    fn register_irq(irq: &IrqInfo, handler: BIrqHandler) {
        info!("Registering IRQ for UART: {:?}", irq);

        static IRQ_REGISTED: AtomicBool = AtomicBool::new(false);
        static IRQ_HANDLER: HWIrqHandler = HWIrqHandler(UnsafeCell::new(None));
        unsafe {
            *IRQ_HANDLER.0.get() = Some(handler);
        }

        if IRQ_REGISTED
            .compare_exchange(
                false,
                true,
                core::sync::atomic::Ordering::SeqCst,
                core::sync::atomic::Ordering::SeqCst,
            )
            .is_ok()
        {
            info!("✓ IRQ not registered yet, proceeding with registration");
            IrqParam {
                intc: irq.irq_parent,
                cfg: irq.cfgs[0].clone(),
            }
            .register_builder(move |_irq| {
                // 增加中断处理函数调用计数
                IRQ_HANDLER_CALL_COUNT.fetch_add(1, Ordering::SeqCst);

                // 清除中断状态并获取触发类型
                let status = unsafe {
                    (*IRQ_HANDLER.0.get())
                        .as_mut()
                        .unwrap()
                        .clean_interrupt_status()
                };

                // 根据中断类型增加相应计数器
                if status.contains(InterruptMask::TX_EMPTY) {
                    TX_INTERRUPT_COUNT.fetch_add(1, Ordering::SeqCst);
                }
                if status.contains(InterruptMask::RX_AVAILABLE) {
                    RX_INTERRUPT_COUNT.fetch_add(1, Ordering::SeqCst);
                }

                IrqHandleResult::Handled
            })
            .register();
            info!("✓ Enhanced IRQ registered: {:?}", irq);
        } else {
            debug!("✓ IRQ already registered, skipping");
        }
    }

    // /// 多数据模式中断测试
    // #[test]
    // fn test_interrupt_multi_pattern() {
    //     info!("=== Interrupt Multi-Pattern Test ===");

    //     let mut serial = create_test_serial();
    //     serial.open().expect("Failed to open serial");

    //     // 配置串口
    //     let config = some_serial::Config::new()
    //         .baudrate(115200)
    //         .data_bits(DataBits::Eight)
    //         .stop_bits(StopBits::One)
    //         .parity(Parity::None);

    //     if let Err(e) = serial.set_config(&config) {
    //         info!("Serial config failed: {:?}", e);
    //         return;
    //     }

    //     // 重置中断计数器
    //     reset_interrupt_counters();
    //     print_interrupt_counts("initial");

    //     // 启用中断
    //     serial.enable_interrupts(InterruptMask::TX_EMPTY | InterruptMask::RX_AVAILABLE);
    //     serial.enable_loopback();

    //     // 测试多种数据模式
    //     let test_patterns: &[(&str, &[u8])] = &[
    //         ("Empty", b""),
    //         ("Single byte", b"A"),
    //         ("Short text", b"Hello"),
    //         ("Medium text", b"This is a medium length test string"),
    //         ("Numbers", b"0123456789"),
    //         ("Special chars", b"!@#$%^&*()"),
    //         ("Binary data", &[0x00, 0x01, 0x7F, 0x80, 0xFF]),
    //         (
    //             "Long data",
    //             b"This is a longer test string to test interrupt handling with larger data amounts",
    //         ),
    //     ];

    //     let mut passed_tests = 0;
    //     let mut total_tests = 0;

    //     for (pattern_name, pattern_data) in test_patterns.iter() {
    //         total_tests += 1;
    //         info!(
    //             "Testing pattern: {} ({} bytes)",
    //             pattern_name,
    //             pattern_data.len()
    //         );

    //         // 获取TX/RX接口
    //         let mut tx = match serial.take_tx() {
    //             Some(tx) => tx,
    //             None => {
    //                 info!("  ✗ Failed to get TX interface");
    //                 continue;
    //             }
    //         };

    //         let mut rx = match serial.take_rx() {
    //             Some(rx) => rx,
    //             None => {
    //                 info!("  ✗ Failed to get RX interface");
    //                 continue;
    //             }
    //         };

    //         // 记录测试前的中断计数
    //         let (tx_before, rx_before, _) = get_interrupt_counts();

    //         // 发送数据
    //         let sent_bytes = tx.send(pattern_data);
    //         info!("  Sent {} bytes", sent_bytes);

    //         // 等待中断处理
    //         for _ in 0..12000 {
    //             core::hint::spin_loop();
    //         }

    //         // 尝试接收数据
    //         let mut recv_buf = vec![0u8; pattern_data.len() + 10];
    //         let receive_success = match rx.recive(&mut recv_buf) {
    //             Ok(received_bytes) => {
    //                 info!("  Received {} bytes", received_bytes);

    //                 if received_bytes == sent_bytes {
    //                     let sent_data = &pattern_data[..sent_bytes];
    //                     let received_data = &recv_buf[..received_bytes];
    //                     sent_data == received_data
    //                 } else {
    //                     false
    //                 }
    //             }
    //             Err(_) => false,
    //         };

    //         // 再次等待中断处理
    //         for _ in 0..8000 {
    //             core::hint::spin_loop();
    //         }

    //         // 检查中断计数变化
    //         let (tx_after, rx_after, _) = get_interrupt_counts();
    //         let tx_triggered = tx_after > tx_before;
    //         let rx_triggered = rx_after > rx_before;

    //         info!(
    //             "  Interrupt activity: TX={}, RX={}",
    //             tx_after - tx_before,
    //             rx_after - rx_before
    //         );

    //         // 评估测试结果
    //         let test_passed = if pattern_data.is_empty() {
    //             // 空数据测试：没有中断触发也算成功
    //             !tx_triggered && !rx_triggered
    //         } else {
    //             // 非空数据测试：应该有中断触发且数据正确
    //             (tx_triggered || rx_triggered) && receive_success
    //         };

    //         if test_passed {
    //             info!("  ✓ Pattern test passed");
    //             passed_tests += 1;
    //         } else {
    //             info!("  ✗ Pattern test failed");
    //         }

    //         // 清理资源
    //         drop(tx);
    //         drop(rx);
    //     }

    //     // 打印最终统计
    //     let (final_tx, final_rx, final_handler) = get_interrupt_counts();
    //     print_interrupt_counts("final");

    //     info!(
    //         "Multi-pattern test results: {}/{} patterns passed",
    //         passed_tests, total_tests
    //     );
    //     info!(
    //         "Final interrupt counts: TX={}, RX={}, Handler={}",
    //         final_tx, final_rx, final_handler
    //     );

    //     // 清理
    //     serial.disable_interrupts(InterruptMask::TX_EMPTY | InterruptMask::RX_AVAILABLE);
    //     serial.disable_loopback();
    //     info!("✓ Interrupt multi-pattern test completed");
    // }

    // /// 中断压力测试
    // #[test]
    // fn test_interrupt_stress() {
    //     info!("=== Interrupt Stress Test ===");

    //     let mut serial = create_test_serial();
    //     serial.enable_loopback();
    //     serial.open().expect("Failed to open serial");

    //     // 配置串口
    //     let config = some_serial::Config::new()
    //         .baudrate(115200)
    //         .data_bits(DataBits::Eight)
    //         .stop_bits(StopBits::One)
    //         .parity(Parity::None);

    //     if let Err(e) = serial.set_config(&config) {
    //         info!("Serial config failed: {:?}", e);
    //         return;
    //     }

    //     // 重置中断计数器
    //     reset_interrupt_counters();
    //     print_interrupt_counts("initial");

    //     // 启用所有中断
    //     serial.enable_interrupts(InterruptMask::TX_EMPTY | InterruptMask::RX_AVAILABLE);

    //     info!("Starting high-frequency interrupt stress test for PL011...");

    //     // 获取TX/RX接口
    //     let mut tx = match serial.take_tx() {
    //         Some(tx) => tx,
    //         None => {
    //             panic!("✗ Failed to get TX interface");
    //         }
    //     };

    //     let mut rx = match serial.take_rx() {
    //         Some(rx) => rx,
    //         None => {
    //             panic!("✗ Failed to get RX interface");
    //         }
    //     };

    //     // 高频数据传输压力测试
    //     let stress_iterations = 50;
    //     let mut total_interrupts = 0;
    //     let mut successful_iterations = 0;

    //     for i in 0..stress_iterations {
    //         // 记录迭代开始时的中断计数
    //         let (tx_before, rx_before, _handler_before) = get_interrupt_counts();

    //         // 快速发送数据
    //         let test_string = format!("Stress iteration {}", i);
    //         let test_data = test_string.as_bytes();
    //         let sent_bytes = tx.send(test_data).unwrap();

    //         // 短暂等待
    //         for _ in 0..2000 {
    //             core::hint::spin_loop();
    //         }

    //         // 尝试接收数据
    //         let mut recv_buf = vec![0u8; test_data.len() + 10];
    //         let receive_success = match rx.recive(&mut recv_buf) {
    //             Ok(received_bytes) => received_bytes > 0 && received_bytes <= sent_bytes,
    //             Err(_) => false,
    //         };

    //         // 再次等待
    //         for _ in 0..2000 {
    //             core::hint::spin_loop();
    //         }

    //         // 检查中断活动
    //         let (tx_after, rx_after, _handler_after) = get_interrupt_counts();
    //         let iteration_interrupts = (tx_after - tx_before) + (rx_after - rx_before);
    //         total_interrupts += iteration_interrupts;

    //         // 评估迭代成功
    //         if iteration_interrupts > 0 && receive_success {
    //             successful_iterations += 1;
    //         }

    //         // 每10次迭代打印一次进度
    //         if (i + 1) % 10 == 0 {
    //             info!(
    //                 "Stress progress: {}/{} iterations, {}/{} successful, {} total interrupts",
    //                 i + 1,
    //                 stress_iterations,
    //                 successful_iterations,
    //                 i + 1,
    //                 total_interrupts
    //             );
    //         }
    //     }

    //     // 最终统计
    //     let (final_tx, final_rx, final_handler) = get_interrupt_counts();
    //     print_interrupt_counts("final");

    //     info!("Stress test results:");
    //     info!("  Total iterations: {}", stress_iterations);
    //     info!("  Successful iterations: {}", successful_iterations);
    //     info!(
    //         "  Success rate: {:.1}%",
    //         (successful_iterations as f64 / stress_iterations as f64) * 100.0
    //     );
    //     info!("  Total interrupts: {}", total_interrupts);
    //     info!(
    //         "  Average interrupts per iteration: {:.1}",
    //         total_interrupts as f64 / stress_iterations as f64
    //     );
    //     info!(
    //         "  Final counts: TX={}, RX={}, Handler={}",
    //         final_tx, final_rx, final_handler
    //     );

    //     // 验证压力测试结果
    //     if successful_iterations >= (stress_iterations / 2) {
    //         info!("✓ PL011 interrupt stress test passed - adequate performance under load");
    //     } else {
    //         panic!("✗ PL011 interrupt stress test failed - poor performance under load");
    //     }

    //     if total_interrupts > stress_iterations {
    //         info!("✓ Sufficient interrupt activity detected");
    //     } else {
    //         panic!("✗ Insufficient interrupt activity detected");
    //     }

    //     // 清理
    //     drop(tx);
    //     drop(rx);
    //     info!("✓  interrupt stress test completed");

    //     // 最终清理
    //     serial.disable_interrupts(InterruptMask::TX_EMPTY | InterruptMask::RX_AVAILABLE);
    //     serial.disable_loopback();
    //     info!("=== Interrupt Stress Test Completed ===");
    // }

    // /// 综合中断测试套件
    // #[test]
    // fn test_interrupt_comprehensive_suite() {
    //     info!("=== Comprehensive Interrupt Test Suite ===");

    //     let mut test_results = Vec::new();
    //     let total_tests = 5;

    //     // 测试1：TX中断基础功能
    //     info!("Running TX interrupt basic test...");
    //     let mut serial = create_test_serial();
    //     serial.open().expect("Failed to open serial");
    //     reset_interrupt_counters();
    //     serial.enable_interrupts(InterruptMask::TX_EMPTY);
    //     let mut tx = serial.take_tx().unwrap();
    //     tx.send(b"Comprehensive TX test");
    //     drop(tx);
    //     for _ in 0..5000 {
    //         core::hint::spin_loop();
    //     }
    //     let (tx_count, _, _) = get_interrupt_counts();
    //     let tx_test_passed = tx_count > 0;
    //     test_results.push(("TX Basic", tx_test_passed));
    //     info!(
    //         "TX basic test: {}",
    //         if tx_test_passed { "PASSED" } else { "FAILED" }
    //     );

    //     // 测试2：RX中断基础功能
    //     info!("Running RX interrupt basic test...");
    //     reset_interrupt_counters();
    //     serial.enable_interrupts(InterruptMask::RX_AVAILABLE);
    //     serial.enable_loopback();
    //     let mut tx = serial.take_tx().unwrap();
    //     let mut rx = serial.take_rx().unwrap();
    //     tx.send(b"Comprehensive RX test");
    //     drop(tx);
    //     for _ in 0..5000 {
    //         core::hint::spin_loop();
    //     }
    //     let mut recv_buf = vec![0u8; 20];
    //     let _ = rx.recive(&mut recv_buf);
    //     drop(rx);
    //     for _ in 0..5000 {
    //         core::hint::spin_loop();
    //     }
    //     let (_, rx_count, _) = get_interrupt_counts();
    //     let rx_test_passed = rx_count > 0;
    //     test_results.push(("RX Basic", rx_test_passed));
    //     info!(
    //         "RX basic test: {}",
    //         if rx_test_passed { "PASSED" } else { "FAILED" }
    //     );

    //     // 测试3：中断掩码控制
    //     info!("Running interrupt mask control test...");
    //     reset_interrupt_counters();
    //     serial.enable_interrupts(InterruptMask::TX_EMPTY);
    //     let mut tx = serial.take_tx().unwrap();
    //     tx.send(b"Mask test TX only");
    //     drop(tx);
    //     for _ in 0..5000 {
    //         core::hint::spin_loop();
    //     }
    //     let (tx_mask_count, rx_mask_count, _) = get_interrupt_counts();
    //     let mask_test_passed = tx_mask_count > 0 && rx_mask_count == 0;
    //     test_results.push(("Mask Control", mask_test_passed));
    //     info!(
    //         "Mask control test: {}",
    //         if mask_test_passed { "PASSED" } else { "FAILED" }
    //     );

    //     // 测试4：中断处理程序调用
    //     info!("Running interrupt handler test...");
    //     let handler_calls_count = IRQ_HANDLER_CALL_COUNT.load(core::sync::atomic::Ordering::SeqCst);
    //     let handler_test_passed = handler_calls_count > 0;
    //     test_results.push(("Handler Calls", handler_test_passed));
    //     info!(
    //         "Handler calls test: {} (total calls: {})",
    //         if handler_test_passed {
    //             "PASSED"
    //         } else {
    //             "FAILED"
    //         },
    //         handler_calls_count
    //     );

    //     // 测试5：数据传输与中断集成
    //     info!("Running data transfer integration test...");
    //     reset_interrupt_counters();
    //     serial.enable_interrupts(InterruptMask::TX_EMPTY | InterruptMask::RX_AVAILABLE);
    //     let mut tx = serial.take_tx().unwrap();
    //     let mut rx = serial.take_rx().unwrap();
    //     tx.send(b"Integration test final");
    //     drop(tx);
    //     for _ in 0..5000 {
    //         core::hint::spin_loop();
    //     }
    //     let mut recv_buf = vec![0u8; 25];
    //     let _ = rx.recive(&mut recv_buf);
    //     drop(rx);
    //     for _ in 0..5000 {
    //         core::hint::spin_loop();
    //     }
    //     let (final_tx, final_rx, _) = get_interrupt_counts();
    //     let integration_test_passed = final_tx > 0 || final_rx > 0;
    //     test_results.push(("Integration", integration_test_passed));
    //     info!(
    //         "Integration test: {}",
    //         if integration_test_passed {
    //             "PASSED"
    //         } else {
    //             "FAILED"
    //         }
    //     );

    //     // 最终清理
    //     serial.disable_interrupts(InterruptMask::TX_EMPTY | InterruptMask::RX_AVAILABLE);
    //     serial.disable_loopback();

    //     // 统计结果
    //     let passed_tests = test_results.iter().filter(|(_, passed)| *passed).count();
    //     let pass_rate = (passed_tests as f64 / total_tests as f64) * 100.0;

    //     info!("=== Comprehensive Interrupt Test Suite Results ===");
    //     for (test_name, passed) in &test_results {
    //         info!(
    //             "  {}: {}",
    //             test_name,
    //             if *passed { "PASSED" } else { "FAILED" }
    //         );
    //     }
    //     info!(
    //         "Summary: {}/{} tests passed ({:.1}%)",
    //         passed_tests, total_tests, pass_rate
    //     );

    //     if pass_rate >= 80.0 {
    //         info!("✓ Comprehensive interrupt test suite PASSED");
    //     } else {
    //         info!("✗ Comprehensive interrupt test suite FAILED");
    //     }

    //     info!("✓ Comprehensive interrupt test suite completed");
    // }
}
