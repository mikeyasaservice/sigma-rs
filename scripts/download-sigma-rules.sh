#!/bin/bash
# Download and organize Sigma rules from SigmaHQ repository

set -e

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# Configuration
RULES_DIR="./rules"
SIGMA_REPO="https://github.com/SigmaHQ/sigma.git"
TEMP_DIR="sigma-temp"

# Parse arguments
CATEGORY=${1:-all}
FORCE=${2:-false}

# Function to download rules
download_rules() {
    echo -e "${YELLOW}Downloading Sigma rules from GitHub...${NC}"
    
    if [ -d "$TEMP_DIR" ]; then
        echo "Updating existing repository..."
        cd "$TEMP_DIR"
        git pull --depth 1
        cd ..
    else
        echo "Cloning repository..."
        git clone --depth 1 "$SIGMA_REPO" "$TEMP_DIR"
    fi
    
    echo -e "${GREEN}✓ Rules downloaded${NC}"
}

# Function to organize rules by category
organize_rules() {
    local category="$1"
    
    echo -e "${YELLOW}Organizing rules...${NC}"
    
    # Backup existing rules
    if [ -d "$RULES_DIR" ] && [ "$(ls -A $RULES_DIR)" ]; then
        echo "Backing up existing rules..."
        mkdir -p "$RULES_DIR-backup-$(date +%Y%m%d-%H%M%S)"
        cp -r "$RULES_DIR"/* "$RULES_DIR-backup-$(date +%Y%m%d-%H%M%S)/" 2>/dev/null || true
    fi
    
    # Create rules directory structure
    mkdir -p "$RULES_DIR"
    
    case "$category" in
        windows)
            echo "Copying Windows rules..."
            cp -r "$TEMP_DIR/rules/windows" "$RULES_DIR/"
            ;;
        linux)
            echo "Copying Linux rules..."
            cp -r "$TEMP_DIR/rules/linux" "$RULES_DIR/"
            ;;
        cloud)
            echo "Copying Cloud rules..."
            cp -r "$TEMP_DIR/rules/cloud" "$RULES_DIR/"
            ;;
        network)
            echo "Copying Network rules..."
            cp -r "$TEMP_DIR/rules/network" "$RULES_DIR/"
            ;;
        web)
            echo "Copying Web rules..."
            cp -r "$TEMP_DIR/rules/web" "$RULES_DIR/"
            ;;
        critical)
            echo "Copying critical/high severity rules..."
            mkdir -p "$RULES_DIR/critical"
            # Find and copy high/critical severity rules
            find "$TEMP_DIR/rules" -name "*.yml" -exec grep -l "level: critical\|level: high" {} \; | \
                xargs -I {} cp {} "$RULES_DIR/critical/" 2>/dev/null || true
            ;;
        minimal)
            echo "Copying minimal rule set for testing..."
            mkdir -p "$RULES_DIR/minimal"
            # Copy a small subset of commonly used rules
            find "$TEMP_DIR/rules/windows/process_creation" -name "*powershell*.yml" | head -20 | \
                xargs -I {} cp {} "$RULES_DIR/minimal/" 2>/dev/null || true
            find "$TEMP_DIR/rules/windows/process_creation" -name "*mimikatz*.yml" | head -10 | \
                xargs -I {} cp {} "$RULES_DIR/minimal/" 2>/dev/null || true
            ;;
        all)
            echo "Copying all rules..."
            cp -r "$TEMP_DIR/rules/"* "$RULES_DIR/" 2>/dev/null || true
            ;;
        *)
            echo -e "${RED}Unknown category: $category${NC}"
            echo "Valid categories: windows, linux, cloud, network, web, critical, minimal, all"
            exit 1
            ;;
    esac
    
    echo -e "${GREEN}✓ Rules organized${NC}"
}

# Function to validate rules
validate_rules() {
    echo -e "${YELLOW}Validating rules...${NC}"
    
    local total_rules=$(find "$RULES_DIR" -name "*.yml" 2>/dev/null | wc -l | tr -d ' ')
    
    if [ "$total_rules" -eq 0 ]; then
        echo -e "${RED}✗ No rules found in $RULES_DIR${NC}"
        return 1
    fi
    
    echo "Found $total_rules rule files"
    
    # Test loading with sigma-rs if available
    if [ -f "./target/release/sigma-rs" ]; then
        echo "Testing rule loading with sigma-rs..."
        local loaded=$(timeout 5 ./target/release/sigma-rs --rules "$RULES_DIR" < /dev/null 2>&1 | grep "Loaded" | tail -1 || echo "Loaded 0 rules")
        echo "$loaded"
        
        # Parse the number of loaded rules
        local loaded_count=$(echo "$loaded" | grep -o '[0-9]*' | head -1)
        if [ -n "$loaded_count" ] && [ "$loaded_count" -gt 0 ]; then
            local percent=$((loaded_count * 100 / total_rules))
            echo -e "${GREEN}✓ Successfully loaded $loaded_count/$total_rules rules ($percent%)${NC}"
        else
            echo -e "${YELLOW}⚠ Could not determine loaded rules count${NC}"
        fi
    else
        echo -e "${YELLOW}⚠ sigma-rs not built, skipping validation${NC}"
    fi
}

# Function to show statistics
show_statistics() {
    echo ""
    echo -e "${BLUE}Rule Statistics:${NC}"
    echo "========================================="
    
    if [ -d "$RULES_DIR/windows" ]; then
        echo "Windows rules: $(find "$RULES_DIR/windows" -name "*.yml" 2>/dev/null | wc -l | tr -d ' ')"
    fi
    
    if [ -d "$RULES_DIR/linux" ]; then
        echo "Linux rules: $(find "$RULES_DIR/linux" -name "*.yml" 2>/dev/null | wc -l | tr -d ' ')"
    fi
    
    if [ -d "$RULES_DIR/cloud" ]; then
        echo "Cloud rules: $(find "$RULES_DIR/cloud" -name "*.yml" 2>/dev/null | wc -l | tr -d ' ')"
    fi
    
    if [ -d "$RULES_DIR/network" ]; then
        echo "Network rules: $(find "$RULES_DIR/network" -name "*.yml" 2>/dev/null | wc -l | tr -d ' ')"
    fi
    
    if [ -d "$RULES_DIR/web" ]; then
        echo "Web rules: $(find "$RULES_DIR/web" -name "*.yml" 2>/dev/null | wc -l | tr -d ' ')"
    fi
    
    echo ""
    echo "Total rules: $(find "$RULES_DIR" -name "*.yml" 2>/dev/null | wc -l | tr -d ' ')"
}

# Main function
main() {
    echo "========================================="
    echo -e "${BLUE}Sigma Rules Download and Setup${NC}"
    echo "========================================="
    echo ""
    
    # Download rules
    download_rules
    
    # Organize rules by category
    organize_rules "$CATEGORY"
    
    # Validate rules
    validate_rules
    
    # Show statistics
    show_statistics
    
    # Cleanup
    if [ "$FORCE" != "keep-temp" ]; then
        echo -e "${YELLOW}Cleaning up temporary files...${NC}"
        rm -rf "$TEMP_DIR"
    fi
    
    echo ""
    echo "========================================="
    echo -e "${GREEN}✓ Setup complete!${NC}"
    echo "========================================="
    echo ""
    echo "Next steps:"
    echo "  1. Test with: ./target/release/sigma-rs --rules $RULES_DIR"
    echo "  2. Start service: ./scripts/start-service.sh"
    echo "  3. Send test events to evaluate"
    echo ""
}

# Show help
show_help() {
    echo "Usage: $0 [CATEGORY] [OPTIONS]"
    echo ""
    echo "Categories:"
    echo "  windows   - Windows rules only"
    echo "  linux     - Linux rules only"
    echo "  cloud     - Cloud provider rules"
    echo "  network   - Network-based rules"
    echo "  web       - Web application rules"
    echo "  critical  - High/Critical severity rules only"
    echo "  minimal   - Small subset for testing"
    echo "  all       - All available rules (default)"
    echo ""
    echo "Options:"
    echo "  keep-temp - Keep temporary directory after download"
    echo ""
    echo "Examples:"
    echo "  $0 windows      # Download Windows rules only"
    echo "  $0 minimal      # Download minimal test set"
    echo "  $0 all          # Download all rules"
    echo ""
}

# Parse help flag
if [ "$1" = "--help" ] || [ "$1" = "-h" ]; then
    show_help
    exit 0
fi

# Run main function
main