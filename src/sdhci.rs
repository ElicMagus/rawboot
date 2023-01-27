use crate::gpio::{self, GpioRegT};

struct sdhci_cmd_t {
	cmdidx:u32,
	cmdarg:u32,
	resptype:u32,
	response:[u32;4],
}

struct sdhci_data_t {
	buf:*mut u8, //указатель на буфер
	flag:u32,
	blksz:u32,
	blkcnt:u32,
}

struct sdhci_t
{
	name:char, //указатель
	addr:u32,
	pclk:u32,
	reset:u32,
	voltage:u32,
	width:u32,
	clock:u32,
	removable:bool,
	isspi:bool,

	gpio_d0:*mut GpioRegT,
	gpio_d1:*mut GpioRegT,
	gpio_d2:*mut GpioRegT,
	gpio_d3:*mut GpioRegT,
	gpio_cmd:*mut GpioRegT,
	gpio_clk:*mut GpioRegT,

	sdcard:*mut u32, //адрес по которому хранится структура для SD карты
}