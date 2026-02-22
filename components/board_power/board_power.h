#ifndef BOARD_POWER_H
#define BOARD_POWER_H

#include "driver/i2c.h"
#include "esp_err.h"
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

esp_err_t board_power_init(void);
esp_err_t board_ioexpander_lcd_reset(i2c_port_t port);
esp_err_t board_ioexpander_set_pa(uint8_t enable);

#ifdef __cplusplus
}
#endif

#endif
