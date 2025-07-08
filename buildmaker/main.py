import tkinter as tk
from tkinter import ttk
import os
from PIL import Image, ImageTk
from tkinter import simpledialog
import ttkbootstrap as ttk
from ttkbootstrap.constants import *  # For bootstrap constants
import sys

class GroupFrame(ttk.LabelFrame):
    def __init__(self, parent, title="New Group"):
        super().__init__(parent, text=title)
        self.pack(fill=tk.X, padx=5, pady=5, expand=True)
        
        # Container for items
        self.items_frame = ttk.Frame(self)
        self.items_frame.pack(fill=tk.BOTH, expand=True)
        
        # Add right-click menu for group
        self.bind("<Button-3>", self.show_group_menu)
    
    def show_group_menu(self, event):
        menu = tk.Menu(self, tearoff=0)
        menu.add_command(label="Rename", command=self.rename_group)
        menu.add_command(label="Delete", command=self.destroy)
        menu.post(event.x_root, event.y_root)
    
    def rename_group(self):
        new_name = simpledialog.askstring("Rename Group", "Enter new name:", initialvalue=self['text'])
        if new_name:
            self['text'] = new_name

class BuildMakerApp:
    def __init__(self, root):
        self.root = root
        self.root.title("Northgard Build Maker")
        
        # Configure main window
        self.root.geometry("1200x800")
        
        # Set window icon - handle both development and PyInstaller paths
        def resource_path(relative_path):
            try:
                # PyInstaller creates a temp folder and stores path in _MEIPASS
                base_path = sys._MEIPASS
            except Exception:
                base_path = os.path.abspath(".")
            return os.path.join(base_path, relative_path)
        
        icon_path = resource_path("app_icon.ico")
        if os.path.exists(icon_path):
            self.root.iconbitmap(icon_path)
        
        # Apply theme - remove ThemedStyle and use ttkbootstrap's style
        self.style = ttk.Style(theme="darkly")
        
        # Initialize items storage
        self.items_by_category = {
            "buildings": [],
            "lores": [],
            "units": []
        }
        
        # Initialize groups storage with default "800" group
        self.groups = {
            "800": {
                "buildings": [],
                "units": [],
                "description": ""  # Add description to group data
            }
        }
        self.current_group = "800"
        
        # Initialize separate lore order list
        self.lore_order = []
        
        # Load clan lores
        self.load_clan_lores()
        
        # Create menu bar
        self.create_menu()
        
        # Create group management panel
        self.create_group_panel()
        
        # Create main content
        self.create_main_content()
        
        # Load initial items
        self.load_items()
        
        # Initialize the default group in the listbox
        self.group_listbox.insert(tk.END, "800")
        self.group_listbox.selection_set(0)  # Select the default group
        
        # Load the default group state
        self.load_group_state("800")  # Add this line to load the default group state
        
        # Bind clan selection to lore update
        self.clan_var.trace_add('write', self.update_available_lores)
        
        # Update lores for default clan
        self.update_available_lores()
    
    def create_menu(self):
        menubar = tk.Menu(self.root)
        self.root.config(menu=menubar)
        
        # File menu
        file_menu = tk.Menu(menubar, tearoff=0)
        menubar.add_cascade(label="File", menu=file_menu)
        file_menu.add_command(label="New Build", command=self.new_build)
        file_menu.add_command(label="Save Build", command=self.save_build)
        file_menu.add_command(label="Load Build", command=self.load_build)
        file_menu.add_separator()
        file_menu.add_command(label="Exit", command=self.root.quit)
        
        # About menu
        menubar.add_command(label="About", command=self.show_about)
    
    def create_main_content(self):
        # Create main horizontal split
        self.paned_window = ttk.PanedWindow(self.root, orient=tk.HORIZONTAL)
        self.paned_window.pack(fill=tk.BOTH, expand=True)
        
        # Left panel (available items)
        self.left_frame = ttk.Frame(self.paned_window)
        self.paned_window.add(self.left_frame, weight=1)
        
        # Right panel (selected items)
        self.right_frame = ttk.Frame(self.paned_window)
        self.paned_window.add(self.right_frame, weight=1)
        
        # Create tabs for both panels
        self.create_tabs(self.left_frame, "available")
        self.create_tabs(self.right_frame, "selected")
    
    def create_tabs(self, parent, prefix):
        notebook = ttk.Notebook(parent)
        notebook.pack(fill=tk.BOTH, expand=True)
        
        # Create tabs for buildings, lores, and units
        tabs = {}
        for category in ["buildings", "lores", "units"]:
            frame = ttk.Frame(notebook)
            
            # Add search bar if this is the available panel
            if prefix == "available":
                search_frame = ttk.Frame(frame)
                search_frame.pack(fill=tk.X, padx=5, pady=5)
                
                search_label = ttk.Label(search_frame, text="Search:")
                search_label.pack(side=tk.LEFT, padx=(0, 5))
                
                search_var = tk.StringVar()
                search_entry = ttk.Entry(search_frame, textvariable=search_var)
                search_entry.pack(side=tk.LEFT, fill=tk.X, expand=True)
                
                clear_button = ttk.Button(
                    search_frame,
                    text="✕",
                    width=3,
                    command=lambda v=search_var: v.set("")
                )
                clear_button.pack(side=tk.LEFT, padx=(5, 0))
                
                # Bind search update
                search_var.trace_add('write', lambda *args, c=category, v=search_var: 
                    self.filter_items(c, v.get()))
            
            # Create treeview with scrollbar
            frame_inner = ttk.Frame(frame)
            frame_inner.pack(fill=tk.BOTH, expand=True, padx=5, pady=5)
            
            tree = ttk.Treeview(frame_inner, columns=(), show="tree", height=10)
            scrollbar = ttk.Scrollbar(frame_inner, orient=tk.VERTICAL, command=tree.yview)
            tree.configure(yscrollcommand=scrollbar.set)
            
            # Configure row height
            tree.configure(style="Custom.Treeview")
            self.style.configure("Custom.Treeview", rowheight=40)  # Adjust this value as needed
            
            tree.pack(side=tk.LEFT, fill=tk.BOTH, expand=True)
            scrollbar.pack(side=tk.RIGHT, fill=tk.Y)
            
            # Store treeview reference
            if prefix == "available":
                tabs[category] = tree
            else:
                tabs[category] = tree
                
                # Add right-click menu for the right panel
                if category != "lores":
                    tree.bind("<Button-3>", 
                        lambda e, t=tree: self.show_panel_context_menu(e, t))
            
            # Bind double-click
            tree.bind('<Double-Button-1>', self.handle_double_click)
            tree.bind('<Button-3>', self.show_context_menu)
            
            notebook.add(frame, text=category.capitalize())
        
        setattr(self, f"{prefix}_tabs", tabs)
    
    def load_items(self):
        # Load items from resource folders
        for category in ["buildings", "units"]:  # Remove "lores" from here
            folder_path = os.path.join("res", category)
            if os.path.exists(folder_path):
                for file in os.listdir(folder_path):
                    if file.lower().endswith(('.png', '.jpg', '.jpeg', '.gif')):
                        self.create_item_widget(
                            category,
                            file,
                            self.available_tabs[category]
                        )
                        self.items_by_category[category].append({
                            'filename': file,
                            'name': os.path.splitext(file)[0].replace('_', ' ').replace('-', ' ').title()
                        })
    
    def create_item_widget(self, category, filename, target_tree=None):
        image_path = os.path.join("res", category, filename)
        try:
            image = Image.open(image_path)
            image = image.resize((32, 32), Image.Resampling.LANCZOS)
            photo = ImageTk.PhotoImage(image)
            
            # Store image reference in a dictionary that won't be garbage collected
            if not hasattr(self, 'item_images'):
                self.item_images = {}
            
            # Use a unique key for each instance of the image
            if target_tree:
                # Add timestamp to make the key unique for each instance
                key = f"{category}_{filename}_{id(target_tree)}_{id(photo)}"
            else:
                key = f"{category}_{filename}"
            
            self.item_images[key] = photo
            
            name = os.path.splitext(filename)[0].replace('_', ' ').replace('-', ' ').title()
            
            if target_tree:
                item_id = target_tree.insert("", "end", text=name, image=self.item_images[key])
                
                # Store item data
                if not hasattr(target_tree, 'item_data'):
                    target_tree.item_data = {}
                target_tree.item_data[item_id] = {
                    'category': category,
                    'filename': filename,
                    'name': name,
                    'image_key': key  # Store the image key for reference
                }
                
            return {
                'photo': photo,
                'name': name,
                'filename': filename
            }
            
        except Exception as e:
            print(f"Error loading image {filename}: {e}")
            return None
    
    def move_item(self, item, target_frame):
        if target_frame == item.master:
            return
            
        if item.master in self.available_tabs.values():
            if self.current_group:  # Only allow adding items when a group is selected
                # Create copy in the target frame
                self.create_item_widget(
                    item.category,
                    item.filename,
                    target_frame
                )
                # Save state immediately after adding
                self.save_current_group_state()
    
    def handle_double_click(self, event):
        # Get the treeview that triggered the event
        tree = event.widget
        selection = tree.selection()  # Get selected item ID
        if not selection:
            return
        
        # Get the selected item's data
        item_id = selection[0]
        item_data = tree.item_data[item_id]
        
        # Only handle double-click for moving from available to selected panel
        if tree in self.available_tabs.values():
            target_tree = self.selected_tabs[item_data['category']]
            self.create_item_widget(
                item_data['category'],
                item_data['filename'],
                target_tree
            )
            
            # If it's a lore, remove it from available list and update available lores
            if item_data['category'] == 'lores':
                tree.delete(item_id)
                self.update_available_lores()  # Update available lores
            
            # Save state immediately after adding
            self.save_current_group_state()
    
    def show_context_menu(self, event):
        # Get the treeview that triggered the event
        tree = event.widget
        selection = tree.selection()  # Get selected item ID
        if not selection:
            return
        
        # Only show context menu for selected items
        if tree in self.selected_tabs.values():
            menu = tk.Menu(self.root, tearoff=0)
            menu.add_command(
                label="Remove",
                command=lambda: self.remove_item(tree, selection[0])
            )
            menu.post(event.x_root, event.y_root)
    
    def remove_item(self, tree, item_id):
        item_data = tree.item_data[item_id]
        
        # Remove from selected panel
        tree.delete(item_id)
        
        # If it's a lore, update available lores
        if item_data['category'] == 'lores':
            self.update_available_lores()  # Update available lores to show newly available lore
        
        # Save state immediately after removing
        self.save_current_group_state()
    
    def new_build(self):
        # Clear buildings and units
        self.clear_selected_items()
        
        # Clear lores separately
        lore_tree = self.selected_tabs["lores"]
        for item_id in lore_tree.get_children():
            lore_tree.delete(item_id)
        self.lore_order = []
        
        # Reset to default group
        self.groups = {
            "800": {
                "buildings": [],
                "units": [],
                "description": ""
            }
        }
        self.current_group = "800"
        
        # Clear and reset listbox
        self.group_listbox.delete(0, tk.END)
        self.group_listbox.insert(tk.END, "800")
        self.group_listbox.selection_set(0)
        
        # Clear description text
        self.description_text.delete("1.0", tk.END)
        
        # Update available lores
        self.update_available_lores()
    
    def save_build(self):
        from tkinter import filedialog
        import json
        import os
        
        file_path = filedialog.asksaveasfilename(
            defaultextension=".json",
            filetypes=[("JSON files", "*.json"), ("All files", "*.*")]
        )
        
        if not file_path:
            return
        
        # Prepare build data
        build_data = {
            "clan": self.clan_var.get(),
            "lore_order": [os.path.splitext(lore)[0] for lore in self.lore_order],  # Remove .png
            "groups": {}
        }
        
        # Save all groups (without lores)
        for group_name, group_data in self.groups.items():
            build_data["groups"][group_name] = {
                "buildings": [os.path.splitext(item["filename"])[0] for item in group_data["buildings"]],
                "units": [os.path.splitext(item["filename"])[0] for item in group_data["units"]],
                "description": group_data.get("description", "")  # Include group description
            }
        
        try:
            with open(file_path, 'w') as f:
                json.dump(build_data, f, indent=4)
        except Exception as e:
            from tkinter import messagebox
            messagebox.showerror("Error", f"Failed to save build: {str(e)}")
    
    def load_build(self):
        from tkinter import filedialog
        import json
        
        file_path = filedialog.askopenfilename(
            filetypes=[("JSON files", "*.json"), ("All files", "*.*")]
        )
        
        if not file_path:
            return
        
        try:
            with open(file_path, 'r') as f:
                build_data = json.load(f)
            
            # Load clan
            if "clan" in build_data:
                self.clan_combo.set(build_data["clan"])
                # Update available lores for the loaded clan
                self.update_available_lores()
            
            # Clear existing groups and lores
            self.groups = {}
            self.lore_order = []
            self.group_listbox.delete(0, tk.END)
            self.clear_selected_items()
            
            # Clear existing lores
            lore_tree = self.selected_tabs["lores"]
            for item_id in lore_tree.get_children():
                lore_tree.delete(item_id)
            
            # Load lore order (add .png extension)
            if "lore_order" in build_data:
                self.lore_order = [f"{lore}.png" for lore in build_data["lore_order"]]
                for lore in self.lore_order:
                    self.create_item_widget("lores", lore, self.selected_tabs["lores"])
                # Update available lores after loading selected lores
                self.update_available_lores()
            
            # Load groups
            for group_name, group_data in build_data["groups"].items():
                self.groups[group_name] = {
                    "buildings": [{"filename": f"{f}.png", "category": "buildings"} 
                                for f in group_data["buildings"]],
                    "units": [{"filename": f"{f}.png", "category": "units"} 
                             for f in group_data["units"]],
                    "description": group_data.get("description", "")
                }
                self.group_listbox.insert(tk.END, group_name)
            
            # Select first group if exists
            if self.group_listbox.size() > 0:
                self.group_listbox.selection_set(0)
                self.current_group = self.group_listbox.get(0)
                self.load_group_state(self.current_group)
                
        except Exception as e:
            from tkinter import messagebox
            messagebox.showerror("Error", f"Failed to load build: {str(e)}")
    
    def show_about(self):
        from tkinter import messagebox
        messagebox.showinfo("About", "Buildmaker for Northgard Assistant v0.0.1")

    def filter_items(self, category, search_text):
        search_text = search_text.lower()
        tree = self.available_tabs[category]
        
        # Clear all items first
        for item_id in tree.get_children():
            tree.delete(item_id)
        
        if category == "lores":
            # Get currently selected lores first
            selected_lores = set()
            selected_tree = self.selected_tabs["lores"]
            for item_id in selected_tree.get_children():
                item_data = selected_tree.item_data[item_id]
                selected_lores.add(os.path.splitext(item_data['filename'])[0])  # Remove .png
            
            # For lores, filter from clan_lores
            selected_clan = self.clan_var.get()
            if selected_clan in self.clan_lores:
                for lore_name in self.clan_lores[selected_clan]:
                    # Only show if not already selected AND matches search
                    if lore_name not in selected_lores and search_text in lore_name.lower():
                        filename = f"{lore_name}.png"
                        self.create_item_widget("lores", filename, tree)
        else:
            # For buildings and units, filter from items_by_category
            for item in self.items_by_category[category]:
                if search_text in item['name'].lower():
                    self.create_item_widget(
                        category,
                        item['filename'],
                        tree
                    )

    def show_panel_context_menu(self, event, frame):
        menu = tk.Menu(self.root, tearoff=0)
        menu.add_command(label="Create Group", 
            command=lambda: self.create_group(frame))
        menu.post(event.x_root, event.y_root)
    
    def create_group(self, parent):
        name = simpledialog.askstring("New Group", "Enter group name:")
        if name:
            group = GroupFrame(parent, name)
            # Bind right-click menu to the group
            group.bind("<Button-3>", lambda e: self.show_group_menu(e, group))
    
    def show_group_menu(self, event, group):
        menu = tk.Menu(self.root, tearoff=0)
        menu.add_command(label="Rename Group", 
            command=lambda: self.rename_group(group))
        menu.add_command(label="Delete Group", 
            command=lambda: self.delete_group(group))
        menu.post(event.x_root, event.y_root)
    
    def rename_group(self, group):
        new_name = simpledialog.askstring("Rename Group", 
            "Enter new name:", initialvalue=group['text'])
        if new_name:
            group['text'] = new_name
    
    def delete_group(self, group):
        # Move items back to main frame if needed or just destroy
        group.destroy()

    def create_group_panel(self):
        # Create top panel for group management
        group_panel = ttk.Frame(self.root)
        group_panel.pack(fill=tk.X, padx=5, pady=5)
        
        # Left side - clan selection and listbox
        left_frame = ttk.Frame(group_panel)
        left_frame.pack(side=tk.LEFT, fill=tk.BOTH, expand=True)
        
        # Add clan selection
        clan_frame = ttk.Frame(left_frame)
        clan_frame.pack(fill=tk.X, pady=(0, 5))
        
        # Update clan label style
        clan_label = ttk.Label(clan_frame, text="Choose clan:")
        clan_label.pack(side=tk.LEFT, padx=(5, 5))
        
        self.clan_var = tk.StringVar()
        self.clan_combo = ttk.Combobox(clan_frame, textvariable=self.clan_var)
        self.clan_combo['values'] = [
            "Bear", "Boar", "Dragon", "Eagle", "Goat", 
            "Horse", "Hound", "Kraken", "Lion", "Lynx",
            "Ox", "Rat", "Raven", "Snake", "Squirrel",
            "Stag", "Stoat", "Turtle", "Wolf"
        ]
        self.clan_combo.pack(side=tk.LEFT, fill=tk.X, expand=True)
        self.clan_combo.set("Stag")  # Default clan
        
        # Update groups label style
        groups_label = ttk.Label(left_frame, text="Groups:")
        groups_label.pack(anchor=tk.W, padx=5, pady=(0, 2))
        
        # Configure listbox with simpler style
        self.group_listbox = tk.Listbox(
            left_frame, 
            height=3,
            font=("Segoe UI", 9),
            selectmode=tk.SINGLE,
            activestyle='none'  # Remove dotted line around selected item
        )
        self.group_listbox.pack(fill=tk.BOTH, expand=True)
        self.group_listbox.bind('<<ListboxSelect>>', self.on_group_select)
        
        # Right side - group controls and description
        controls_frame = ttk.Frame(group_panel)
        controls_frame.pack(side=tk.LEFT, padx=(10, 0), fill=tk.BOTH)
        
        # Group name controls
        name_label = ttk.Label(controls_frame, text="Group name:", font=("", 10))
        name_label.pack(anchor=tk.W, padx=5, pady=(0, 2))
        
        self.group_entry = ttk.Entry(controls_frame)
        self.group_entry.pack(fill=tk.X, padx=5, pady=(0, 5))
        
        # Buttons frame
        buttons_frame = ttk.Frame(controls_frame)
        buttons_frame.pack(fill=tk.X)
        ttk.Button(buttons_frame, text="Add", command=self.create_new_group).pack(side=tk.LEFT, padx=2)
        ttk.Button(buttons_frame, text="Rename", command=self.rename_current_group).pack(side=tk.LEFT, padx=2)
        ttk.Button(buttons_frame, text="Delete", command=self.delete_current_group).pack(side=tk.LEFT, padx=2)
        
        # Description section
        desc_label = ttk.Label(controls_frame, text="Description:", font=("", 10))
        desc_label.pack(anchor=tk.W, padx=5, pady=(10, 2))
        
        # Configure Text widget with simpler style
        self.description_text = tk.Text(
            controls_frame, 
            height=3, 
            wrap=tk.WORD,
            font=("Segoe UI", 9)
        )
        self.description_text.pack(fill=tk.BOTH, padx=5, expand=True)
        
        # Add binding for description changes
        self.description_text.bind('<<Modified>>', self.on_description_change)

    def rename_current_group(self):
        selection = self.group_listbox.curselection()
        if not selection:
            return
        
        old_name = self.group_listbox.get(selection[0])
        new_name = self.group_entry.get().strip()
        
        if new_name and new_name != old_name and new_name not in self.groups:
            # Update groups dictionary
            self.groups[new_name] = self.groups.pop(old_name)
            
            # Update listbox
            self.group_listbox.delete(selection[0])
            self.group_listbox.insert(selection[0], new_name)
            self.group_listbox.selection_set(selection[0])
            
            # Update current group if it was renamed
            if self.current_group == old_name:
                self.current_group = new_name
            
            # Clear entry
            self.group_entry.delete(0, tk.END)

    def create_new_group(self):
        name = self.group_entry.get().strip()
        if name and name not in self.groups:
            # Create new group
            self.groups[name] = {
                "buildings": [],
                "units": [],
                "description": self.description_text.get("1.0", tk.END).strip()
            }
            
            # Add to listbox and clear entry
            self.group_listbox.insert(tk.END, name)
            self.group_entry.delete(0, tk.END)
            
            # Save current group state before switching
            if self.current_group:
                self.save_current_group_state()
            
            # Set as current group
            self.current_group = name
            
            # Select the new group in listbox
            self.group_listbox.selection_clear(0, tk.END)
            self.group_listbox.selection_set(tk.END)
            
            # Clear selected items
            self.clear_selected_items()
            
            # Clear description
            self.description_text.delete("1.0", tk.END)

    def delete_current_group(self):
        selection = self.group_listbox.curselection()
        if selection:
            name = self.group_listbox.get(selection[0])
            del self.groups[name]
            self.group_listbox.delete(selection[0])
            if name == self.current_group:
                self.current_group = None
                self.clear_selected_items()

    def on_group_select(self, event):
        selection = self.group_listbox.curselection()
        if selection:
            new_group = self.group_listbox.get(selection[0])
            if new_group != self.current_group:
                # Save current description before switching
                if self.current_group:
                    self.groups[self.current_group]["description"] = self.description_text.get("1.0", tk.END).strip()
                
                # Switch to new group
                self.current_group = new_group
                self.load_group_state(new_group)
                
                # Load new group's description
                self.description_text.delete("1.0", tk.END)
                self.description_text.insert("1.0", self.groups[new_group].get("description", ""))
                
                # Update available lores
                self.update_available_lores()

    def save_current_group_state(self):
        if not self.current_group:
            return
            
        group_data = {
            "buildings": [],
            "units": [],
            "description": self.description_text.get("1.0", tk.END).strip()
        }
        
        # Save items from buildings and units
        for category in ["buildings", "units"]:
            tree = self.selected_tabs[category]
            for item_id in tree.get_children():  # Get all items in the tree
                item_data = tree.item_data[item_id]
                group_data[category].append({
                    'filename': item_data['filename'],
                    'category': item_data['category']
                })
        
        self.groups[self.current_group] = group_data
        
        # Save lore order separately
        self.lore_order = []
        lore_tree = self.selected_tabs["lores"]
        for item_id in lore_tree.get_children():
            self.lore_order.append(lore_tree.item_data[item_id]['filename'])

    def load_group_state(self, group_name):
        # Clear only buildings and units, keep lores
        for category in ["buildings", "units"]:
            tree = self.selected_tabs[category]
            for item_id in tree.get_children():
                tree.delete(item_id)
        
        if group_name not in self.groups:
            return
            
        group_data = self.groups[group_name]
        
        # Load items for buildings and units only
        for category in ["buildings", "units"]:
            for item_data in group_data[category]:
                self.create_item_widget(
                    item_data['category'],
                    item_data['filename'],
                    self.selected_tabs[category]
                )
        
        # Load description
        self.description_text.delete("1.0", tk.END)
        if "description" in group_data:
            self.description_text.insert("1.0", group_data["description"])

    def clear_selected_items(self):
        # Clear only buildings and units from right panel, keep lores
        for category in ["buildings", "units"]:
            tree = self.selected_tabs[category]
            for item_id in tree.get_children():
                tree.delete(item_id)

    def on_description_change(self, event=None):
        if self.current_group:
            # Reset the modified flag
            self.description_text.edit_modified(False)
            # Save the current description
            self.groups[self.current_group]["description"] = self.description_text.get("1.0", tk.END).strip()

    def load_clan_lores(self):
        import json
        
        # Load clan lores from JSON
        try:
            with open('clan_lores.json', 'r') as f:
                self.clan_lores = json.load(f)
        except Exception as e:
            print(f"Error loading clan lores: {e}")
            self.clan_lores = {}

    def update_available_lores(self, *args):
        selected_clan = self.clan_var.get()
        lore_tree = self.available_tabs["lores"]
        
        # Get currently selected lores
        selected_lores = set()
        selected_tree = self.selected_tabs["lores"]
        for item_id in selected_tree.get_children():
            item_data = selected_tree.item_data[item_id]
            selected_lores.add(os.path.splitext(item_data['filename'])[0])  # Remove .png
        
        # Clear current available lores
        for item_id in lore_tree.get_children():
            lore_tree.delete(item_id)
        
        # Load only lores available for selected clan that haven't been chosen
        if selected_clan in self.clan_lores:
            for lore_name in self.clan_lores[selected_clan]:
                if lore_name not in selected_lores:  # Only show if not already selected
                    filename = f"{lore_name}.png"
                    self.create_item_widget("lores", filename, lore_tree)

if __name__ == "__main__":
    root = ttk.Window(themename="darkly")  # Use ttkbootstrap's Window instead of tk.Tk
    app = BuildMakerApp(root)
    root.mainloop()
