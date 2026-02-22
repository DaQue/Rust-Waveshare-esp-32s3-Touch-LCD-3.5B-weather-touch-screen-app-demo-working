#include "board_power.h"

#include "esp_check.h"
#include "esp_log.h"
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"

#include "bsp_axp2101.h"
#include "bsp_i2c.h"

#define TCA9554_ADDR 0x20
#define TCA9554_REG_OUTPUT 0x01
#define TCA9554_REG_CONFIG 0x03
#define TCA9554_LCD_RST_BIT (1U << 1)
#define TCA9554_PA_CTRL_BIT (1U << 7)

static const char *TAG = "board_power";

static esp_err_t tca9554_read_reg(i2c_port_t port, uint8_t reg, uint8_t *value)
{
    return i2c_master_write_read_device(
        port,
        TCA9554_ADDR,
        &reg,
        1,
        value,
        1,
        pdMS_TO_TICKS(200));
}

static esp_err_t tca9554_write_reg(i2c_port_t port, uint8_t reg, uint8_t value)
{
    uint8_t payload[2] = {reg, value};
    return i2c_master_write_to_device(
        port,
        TCA9554_ADDR,
        payload,
        sizeof(payload),
        pdMS_TO_TICKS(200));
}

esp_err_t board_ioexpander_lcd_reset(i2c_port_t port)
{
    uint8_t config = 0xFF;
    ESP_RETURN_ON_ERROR(tca9554_read_reg(port, TCA9554_REG_CONFIG, &config), TAG, "read config failed");
    config &= (uint8_t)~TCA9554_LCD_RST_BIT;
    ESP_RETURN_ON_ERROR(tca9554_write_reg(port, TCA9554_REG_CONFIG, config), TAG, "write config failed");

    uint8_t output = 0xFF;
    ESP_RETURN_ON_ERROR(tca9554_read_reg(port, TCA9554_REG_OUTPUT, &output), TAG, "read output failed");

    output &= (uint8_t)~TCA9554_LCD_RST_BIT;
    ESP_RETURN_ON_ERROR(tca9554_write_reg(port, TCA9554_REG_OUTPUT, output), TAG, "assert reset failed");
    vTaskDelay(pdMS_TO_TICKS(100));

    output |= TCA9554_LCD_RST_BIT;
    ESP_RETURN_ON_ERROR(tca9554_write_reg(port, TCA9554_REG_OUTPUT, output), TAG, "deassert reset failed");
    vTaskDelay(pdMS_TO_TICKS(200));

    ESP_LOGI(TAG, "TCA9554 LCD reset pulse complete");
    return ESP_OK;
}

esp_err_t board_ioexpander_set_pa(uint8_t enable)
{
    i2c_port_t port = (i2c_port_t)I2C_PORT_NUM;
    uint8_t config = 0xFF;
    ESP_RETURN_ON_ERROR(tca9554_read_reg(port, TCA9554_REG_CONFIG, &config), TAG, "read config failed");
    config &= (uint8_t)~TCA9554_PA_CTRL_BIT;
    ESP_RETURN_ON_ERROR(tca9554_write_reg(port, TCA9554_REG_CONFIG, config), TAG, "write config failed");

    uint8_t output = 0xFF;
    ESP_RETURN_ON_ERROR(tca9554_read_reg(port, TCA9554_REG_OUTPUT, &output), TAG, "read output failed");
    if (enable) {
        output |= TCA9554_PA_CTRL_BIT;
    } else {
        output &= (uint8_t)~TCA9554_PA_CTRL_BIT;
    }
    ESP_RETURN_ON_ERROR(tca9554_write_reg(port, TCA9554_REG_OUTPUT, output), TAG, "write output failed");

    ESP_LOGI(TAG, "PA control via TCA9554: %s", enable ? "ON" : "OFF");
    return ESP_OK;
}

esp_err_t board_power_init(void)
{
    ESP_RETURN_ON_ERROR(bsp_i2c_init(), TAG, "bsp_i2c_init failed");
    ESP_RETURN_ON_ERROR(bsp_axp2101_init(), TAG, "bsp_axp2101_init failed");
    ESP_RETURN_ON_ERROR(
        board_ioexpander_lcd_reset((i2c_port_t)I2C_PORT_NUM),
        TAG,
        "board_ioexpander_lcd_reset failed");

    ESP_LOGI(TAG, "Board power and LCD reset sequencing complete");
    return ESP_OK;
}
