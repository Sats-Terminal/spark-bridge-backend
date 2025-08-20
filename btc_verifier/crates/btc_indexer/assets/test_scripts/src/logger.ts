import { createLogger, format, transports } from 'winston';

const logger = createLogger({
  level: 'debug',
  format: format.combine(
    format.timestamp(),
    format.printf(
      ({ timestamp, level, message }) => `${timestamp} [${level}]: ${message}`,
    ),
  ),
  transports: [
    new transports.Console(),
    new transports.File({ filename: 'js_tests.log' }),
  ],
});

export default logger;
