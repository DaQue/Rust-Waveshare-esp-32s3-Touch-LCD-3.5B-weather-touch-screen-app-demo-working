#ifndef BSP_I2C_H
#define BSP_I2C_H

#include <stdbool.h>
#include <stdint.h>

#include "driver/gpio.h"
#include "driver/i2c.h"
#include "esp_err.h"

#define EXAMPLE_PIN_I2C_SDA GPIO_NUM_8
#define EXAMPLE_PIN_I2C_SCL GPIO_NUM_7
#define I2C_PORT_NUM 0
#define I2C_FREQ_HZ 400000

#ifdef __cplusplus
extern "C" {
#endif

esp_err_t bsp_i2c_init(void);
bool bsp_i2c_lock(uint32_t timeout_ms);
void bsp_i2c_unlock(void);

#ifdef __cplusplus
}
#endif

#endif
