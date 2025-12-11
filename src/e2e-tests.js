// This file contains the E2E test suite for the application
import { chromium } from 'playwright';

const browser = await chromium.launch();
const page = await browser.newPage();

// Test case to check if the home page loads correctly
await page.goto('http://localhost:3000');
await expect(page).toHaveTitle('Home Page Title');

// Test case to verify navigation from home to about page
await page.click('#about-link');
await expect(page).toHaveURL('/about');

// Test case to check if the contact form submits successfully
await page.fill('#contact-name', 'Test User');
await page.fill('#contact-email', 'test@example.com');
await page.fill('#contact-message', 'This is a test message');
await page.click('#submit-contact-form');
await expect(page).toHaveURL('/thank-you');

// Close the browser after all tests are done
await browser.close();