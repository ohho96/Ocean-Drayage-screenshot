from playwright.sync_api import sync_playwright, TimeoutError
import os
from datetime import datetime
from dotenv import load_dotenv

# Load environment variables
load_dotenv()

# Terminal configuration
TERMINAL_URL = os.getenv('TERMINAL_URL', "https://www.apmterminals.com/en/los-angeles/empty-container-return")
TREMPOINT_URL = os.getenv('TREMPOINT_URL', "https://termpoint.apmterminals.com/")
TERMPOINT_ID = os.getenv('TERMPOINT_ID')
TERMPOINT_PASSWORD = os.getenv('TERMPOINT_PASSWORD')

def take_screenshot(url=TERMINAL_URL, output_dir="screenshots", full_page=True):
    """
    Take a screenshot of a webpage using Playwright.
    
    Args:
        url (str): The URL of the webpage to screenshot
        output_dir (str): Directory to save the screenshot
        full_page (bool): Whether to take a full page screenshot
    """
    print("APM screenshot functionality is currently disabled")
    return
    
    # # Create output directory if it doesn't exist
    # os.makedirs(output_dir, exist_ok=True)
    
    # # Generate filename with timestamp
    # timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    # filename = f"{output_dir}/screenshot_{timestamp}.png"
    
    # with sync_playwright() as p:
    #     # Launch browser
    #     browser = p.chromium.launch()
    #     page = browser.new_page()
        
    #     try:
    #         # Navigate to URL
    #         print(f"Navigating to {url}...")
    #         page.goto(url, wait_until="networkidle")
            
    #         # Take screenshot
    #         print(f"Taking screenshot...")
    #         page.screenshot(path=filename, full_page=full_page)
    #         print(f"Screenshot saved to {filename}")
            
    #     except Exception as e:
    #         print(f"Error occurred: {str(e)}")
        
    #     finally:
    #         browser.close() 

def check_container_return_availability(url=TREMPOINT_URL, container_number="FFAU694237", TERMPOINT_ID=TERMPOINT_ID, TERMPOINT_PASSWORD=TERMPOINT_PASSWORD):
    """
    Login to Termpoint and check for appointment availability.
    
    Args:
        url (str): The Termpoint login URL
        container_number (str): Container number to check
        TERMPOINT_ID (str): Login ID
        TERMPOINT_PASSWORD (str): Login password
    """
    with sync_playwright() as p:
        browser = p.chromium.launch(headless=False)  # Set headless=False to see the browser
        page = browser.new_page()
        
        try:
            # Navigate to login page
            print(f"Navigating to {url}...")
            page.goto(url, wait_until="networkidle", timeout=60000)  # Increased timeout to 60 seconds
            
            # Take screenshot of login page
            print("Taking screenshot of login page...")
            page.screenshot(path="login_page.png")
            
            # Fill in login form
            print("Looking for username field...")
            username_field = page.wait_for_selector('input[name="Login_Nm"]', state='visible', timeout=10000)
            if username_field:
                print("Found username field, waiting for page to stabilize...")
                page.wait_for_timeout(5000)  # Wait 5 seconds for page to stabilize
                
                print("Entering credentials...")
                # Clear fields first
                page.fill('input[name="Login_Nm"]', '')
                page.fill('input[name="Login_Pwd"]', '')
                
                # Wait a bit before entering credentials
                page.wait_for_timeout(2000)
                
                # Enter credentials one by one
                print("Entering username...")
                page.fill('input[name="Login_Nm"]', TERMPOINT_ID)
                page.wait_for_timeout(1000)
                
                print("Entering password...")
                page.fill('input[name="Login_Pwd"]', TERMPOINT_PASSWORD)
                
                # Wait for the page to stabilize
                print("Waiting for the page to stabilize...")
                page.wait_for_timeout(3000)  # Wait 3 seconds
                
                # Click login button
                print("Clicking login button...")
                page.click('button.ui.primary.button.btn-eq-txt.style__LoginButton-s8qvh3d-18.lpgvRJ.components__StyledButton-yks008-0.jAPNmA')
                
                # Wait for navigation after login
                print("Waiting for navigation after login...")
                page.wait_for_load_state("networkidle", timeout=60000)
                
                # Take screenshot after login
                print("Taking screenshot after login...")
                page.screenshot(path="after_login.png")
                
                # Click Schedule an Appointment
                print("Looking for Schedule an Appointment link...")
                schedule_link = page.wait_for_selector('a[href="/ScheduleAppointment"]', state='visible', timeout=10000)
                if schedule_link:
                    print("Found Schedule an Appointment link, clicking...")
                    page.click('a[href="/ScheduleAppointment"]')
                    
                    # Wait for navigation
                    print("Waiting for navigation after clicking Schedule...")
                    page.wait_for_load_state("networkidle", timeout=60000)
                    
                    # Take screenshot
                    print("Taking screenshot after clicking Schedule...")
                    page.screenshot(path="after_schedule_click.png")
                    
                    # Handle dropdown
                    print("Looking for appointment type dropdown...")
                    try:
                        # Wait for the dropdown container
                        dropdown_container = page.wait_for_selector('#ApptTypeDdDv', state='visible', timeout=10000)
                        if dropdown_container:
                            print("Found dropdown container")
                            
                            # Click the dropdown to open it
                            print("Clicking dropdown to open options...")
                            page.click('#ApptTypeDdDv .ui.search.selection.dropdown')
                            
                            # Wait for dropdown options to appear
                            print("Waiting for dropdown options to appear...")
                            page.wait_for_timeout(2000)
                            
                            # Find and click the EMPTY DROPOFF option
                            print("Looking for EMPTY DROPOFF option...")
                            empty_dropoff = page.wait_for_selector('div[radio="container_ero"][name="MD"]', state='visible', timeout=10000)
                            if empty_dropoff:
                                print("Found EMPTY DROPOFF option, clicking...")
                                page.click('div[radio="container_ero"][name="MD"]')
                                
                                # Wait for the selection to be applied
                                print("Waiting for selection to be applied...")
                                page.wait_for_load_state("networkidle", timeout=60000)
                                
                                # Take screenshot after selection
                                print("Taking screenshot after dropdown selection...")
                                page.screenshot(path="after_dropdown_selection.png")
                            else:
                                print("Could not find EMPTY DROPOFF option")
                                page.screenshot(path="option_not_found.png")
                        else:
                            print("Could not find dropdown container")
                            page.screenshot(path="dropdown_not_found.png")
                    except Exception as e:
                        print(f"Error while handling dropdown: {str(e)}")
                        page.screenshot(path="dropdown_error.png")
                    
                    # Handle container number
                    print("Looking for container number textarea...")
                    container_input = page.wait_for_selector('textarea[name="containerName"]', state='visible', timeout=10000)
                    if container_input:
                        print("Found container input, entering container number...")
                        # Clear the textarea first
                        page.fill('textarea[name="containerName"]', '')
                        page.wait_for_timeout(1000)
                        
                        # Enter the container number
                        page.fill('textarea[name="containerName"]', container_number)
                        page.wait_for_timeout(2000)
                        
                        # Submit
                        print("Clicking submit button...")
                        submit_button = page.wait_for_selector('button.ui.primary.button.components__StyledButton-yks008-0.jAPNmA', state='visible', timeout=10000)
                        if submit_button:
                            print("Found submit button, clicking...")
                            page.click('button.ui.primary.button.components__StyledButton-yks008-0.jAPNmA')
                            
                            # Wait for response after submission
                            print("Waiting for response after submission...")
                            page.wait_for_load_state("networkidle", timeout=60000)
                            
                            # Take screenshot after submission
                            print("Taking screenshot after submission...")
                            page.screenshot(path="after_submit_click.png")
                            
                            # Wait for date input to appear
                            print("Waiting for date input to appear...")
                            try:
                                # Wait for the date input container
                                date_container = page.wait_for_selector('div[name="date"][fieldtype="calendar"]', state='visible', timeout=10000)
                                if date_container:
                                    print("Found date container, looking for input...")
                                    # Find and click the calendar icon to open the date picker
                                    calendar_icon = page.wait_for_selector('i.icon-rc-datepicker_calendar', state='visible', timeout=10000)
                                    if calendar_icon:
                                        print("Found calendar icon, clicking...")
                                        page.click('i.icon-rc-datepicker_calendar')
                                        page.wait_for_timeout(2000)
                                        
                                        # Wait for calendar popup to appear
                                        print("Waiting for calendar popup...")
                                        calendar_popup = page.wait_for_selector('.react-datepicker-container', state='visible', timeout=10000)
                                        if calendar_popup:
                                            print("Found calendar popup, selecting today's date...")
                                            # Get today's date
                                            today = datetime.now()
                                            day = today.day
                                            
                                            # Click the day in the calendar
                                            day_selector = f'.react-datepicker-picker.day.current span:text("{day}")'
                                            print(f"Trying to click day with selector: {day_selector}")
                                            page.click(day_selector)
                                            page.wait_for_timeout(2000)
                                            
                                            # Take screenshot after date selection
                                            print("Taking screenshot after date selection...")
                                            page.screenshot(path="after_date_selection.png")
                                            
                                            # Wait for slot dropdown to appear
                                            print("Waiting for slot dropdown to appear...")
                                            slot_dropdown = page.wait_for_selector('#ipgrid_0_slot', state='visible', timeout=10000)
                                            if slot_dropdown:
                                                print("Found slot dropdown, clicking...")
                                                # Click the dropdown to open it
                                                page.click('#ipgrid_0_slot')
                                                page.wait_for_timeout(2000)
                                                
                                                # Wait for the menu to appear
                                                print("Waiting for slot options to appear...")
                                                slot_menu = page.wait_for_selector('#ipgrid_0_slot .menu', state='visible', timeout=10000)
                                                if slot_menu:
                                                    print("Found slot options, taking screenshot...")
                                                    page.screenshot(path="available_slots.png")
                                                    
                                                    # Check if there are any available slots
                                                    slot_items = page.query_selector_all('#ipgrid_0_slot .menu .item')
                                                    if len(slot_items) > 1:  # More than 1 because first item is usually empty
                                                        print("Found available slots")
                                                    else:
                                                        print("No available slots found")
                                                else:
                                                    print("Could not find slot options menu")
                                                    page.screenshot(path="slot_menu_not_found.png")
                                            else:
                                                print("Could not find slot dropdown")
                                                page.screenshot(path="slot_dropdown_not_found.png")
                                        else:
                                            print("Could not find calendar popup")
                                            page.screenshot(path="calendar_popup_not_found.png")
                                    else:
                                        print("Could not find calendar icon")
                                        page.screenshot(path="calendar_icon_not_found.png")
                                else:
                                    print("Could not find date container")
                                    page.screenshot(path="date_container_not_found.png")
                            except Exception as e:
                                print(f"Error while handling date input or slot selection: {str(e)}")
                                page.screenshot(path="date_or_slot_error.png")
                                
                            # Wait for user to see the result
                            print("Waiting for 10 seconds to view the result...")
                            page.wait_for_timeout(10000)
                        else:
                            print("Could not find submit button")
                            page.screenshot(path="submit_button_not_found.png")
                    else:
                        print("Could not find container number textarea")
                        page.screenshot(path="container_input_not_found.png")
                else:
                    print("Could not find Schedule an Appointment link")
            else:
                print("Could not find username field")
                
        except TimeoutError as e:
            print(f"Timeout error occurred: {str(e)}")
            # Take screenshot when timeout occurs
            page.screenshot(path="timeout_error.png")
        except Exception as e:
            print(f"Error occurred: {str(e)}")
            # Take screenshot when error occurs
            page.screenshot(path="error.png")
        finally:
            # Wait a bit before closing
            print("Waiting 5 seconds before closing...")
            page.wait_for_timeout(5000)
            browser.close()

if __name__ == "__main__":
    conatainer_num = "MSMU6479495"
    print("Running testing APM.py for testing purposes with container number: ", conatainer_num)
    print("Checking for appointment availability...")
    check_container_return_availability(container_number=  conatainer_num)
    print("Done")