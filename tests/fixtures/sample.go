package main

import "fmt"

type Config struct {
	Debug bool
	Port  int
}

func NewConfig() *Config {
	return &Config{Debug: false, Port: 8080}
}

func (c *Config) Validate() error {
	if c.Port < 1 || c.Port > 65535 {
		return fmt.Errorf("invalid port: %d", c.Port)
	}
	return nil
}
