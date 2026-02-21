#include "bsp_i2c.h"

#include <assert.h>

#include "esp_check.h"
#include "freertos/FreeRTOS.h"
#include "freertos/semphr.h"

static SemaphoreHandle_t bsp_i2c_mux;
static bool bsp_i2c_initialized;

bool bsp_i2c_lock(uint32_t timeout_ms)
{
    assert(bsp_i2c_mux && "bsp_i2c_init must be called first");

    TickType_t timeout_ticks = (timeout_ms == 0) ? portMAX_DELAY : pdMS_TO_TICKS(timeout_ms);
    return xSemaphoreTakeRecursive(bsp_i2c_mux, timeout_ticks) == pdTRUE;
}

void bsp_i2c_unlock(void)
{
    assert(bsp_i2c_mux && "bsp_i2c_init must be called first");
    xSemaphoreGiveRecursive(bsp_i2c_mux);
}

esp_err_t bsp_i2c_init(void)
{
    if (bsp_i2c_initialized) {
        return ESP_OK;
    }

    i2c_config_t cfg = {
        .mode = I2C_MODE_MASTER,
        .sda_io_num = EXAMPLE_PIN_I2C_SDA,
        .scl_io_num = EXAMPLE_PIN_I2C_SCL,
        .sda_pullup_en = GPIO_PULLUP_ENABLE,
        .scl_pullup_en = GPIO_PULLUP_ENABLE,
        .master.clk_speed = I2C_FREQ_HZ,
        .clk_flags = 0,
    };

    ESP_RETURN_ON_ERROR(i2c_param_config((i2c_port_t)I2C_PORT_NUM, &cfg), "bsp_i2c", "i2c_param_config failed");
    ESP_RETURN_ON_ERROR(
        i2c_driver_install(
            (i2c_port_t)I2C_PORT_NUM,
            cfg.mode,
            0,
            0,
            0),
        "bsp_i2c",
        "i2c_driver_install failed");

    bsp_i2c_mux = xSemaphoreCreateRecursiveMutex();
    if (!bsp_i2c_mux) {
        return ESP_ERR_NO_MEM;
    }

    bsp_i2c_initialized = true;
    return ESP_OK;
}
